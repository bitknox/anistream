// libanistream_mpv.so: the JNI shim between Kotlin's MpvNative and libmpv.
//
// Adapted from mpv-android's app/src/main/jni (MIT, see android/mpv/NOTICE), with one state per
// mpv handle instead of globals, so a second mpv can make seek previews beside the player.
//
// - JNI_OnLoad hands the VM to FFmpeg (av_jni_set_java_vm) for MediaCodec, and registers the
//   natives of tv.anistream.player.MpvNative.
// - attachSurface keeps a global ref to the Surface and gives mpv its address as `wid`, as
//   mpv-android's BaseMPVView does; detachSurface drops it once the VO no longer uses it.
// - Each handle has an event thread that waits on mpv_wait_event and calls the Kotlin listener.
//   Property changes arrive as mpv_node, written out as JSON (UTF-8 bytes, decoded in Kotlin, so
//   a stray byte in a track title cannot abort the VM the way NewStringUTF would).
// - mpv's log goes to logcat under the tag "mpv".

#include <jni.h>
#include <android/log.h>
#include <locale.h>
#include <math.h>
#include <pthread.h>
#include <stdatomic.h>
#include <stdbool.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

#include <libavcodec/jni.h>
#include <mpv/client.h>

#define TAG "mpv"
#define LOGE(...) __android_log_print(ANDROID_LOG_ERROR, TAG, __VA_ARGS__)
#define LOGW(...) __android_log_print(ANDROID_LOG_WARN, TAG, __VA_ARGS__)

static JavaVM *vm;
static jmethodID listener_on_event;
static jobject app_context;

struct client {
    mpv_handle *mpv;
    jobject listener;  // global ref, MpvNative.Listener
    jobject surface;   // global ref, or NULL
    pthread_t thread;
    bool thread_started;
    atomic_bool exiting;
};

static struct client *client_of(jlong handle) {
    return (struct client *)(intptr_t)handle;
}

// --- JSON -----------------------------------------------------------------------------------------

struct buf {
    char *data;
    size_t len, cap;
};

static void buf_put(struct buf *b, const char *s, size_t n) {
    if (b->len + n + 1 > b->cap) {
        size_t cap = b->cap ? b->cap : 256;
        while (cap < b->len + n + 1)
            cap *= 2;
        char *grown = realloc(b->data, cap);
        if (!grown)
            abort();
        b->data = grown;
        b->cap = cap;
    }
    memcpy(b->data + b->len, s, n);
    b->len += n;
    b->data[b->len] = 0;
}

static void buf_str(struct buf *b, const char *s) {
    buf_put(b, s, strlen(s));
}

static void json_string(struct buf *b, const char *s) {
    buf_put(b, "\"", 1);
    for (const unsigned char *p = (const unsigned char *)s; *p; p++) {
        char esc[8];
        switch (*p) {
        case '"': buf_put(b, "\\\"", 2); break;
        case '\\': buf_put(b, "\\\\", 2); break;
        case '\n': buf_put(b, "\\n", 2); break;
        case '\r': buf_put(b, "\\r", 2); break;
        case '\t': buf_put(b, "\\t", 2); break;
        default:
            if (*p < 0x20) {
                snprintf(esc, sizeof esc, "\\u%04x", *p);
                buf_str(b, esc);
            } else {
                buf_put(b, (const char *)p, 1);
            }
        }
    }
    buf_put(b, "\"", 1);
}

static void json_node(struct buf *b, const mpv_node *node) {
    char num[40];
    switch (node->format) {
    case MPV_FORMAT_STRING:
        json_string(b, node->u.string);
        break;
    case MPV_FORMAT_FLAG:
        buf_str(b, node->u.flag ? "true" : "false");
        break;
    case MPV_FORMAT_INT64:
        snprintf(num, sizeof num, "%lld", (long long)node->u.int64);
        buf_str(b, num);
        break;
    case MPV_FORMAT_DOUBLE:
        if (isfinite(node->u.double_)) {
            snprintf(num, sizeof num, "%.17g", node->u.double_);
            buf_str(b, num);
        } else {
            buf_str(b, "null");
        }
        break;
    case MPV_FORMAT_NODE_ARRAY:
        buf_put(b, "[", 1);
        for (int i = 0; i < node->u.list->num; i++) {
            if (i)
                buf_put(b, ",", 1);
            json_node(b, &node->u.list->values[i]);
        }
        buf_put(b, "]", 1);
        break;
    case MPV_FORMAT_NODE_MAP:
        buf_put(b, "{", 1);
        for (int i = 0; i < node->u.list->num; i++) {
            if (i)
                buf_put(b, ",", 1);
            json_string(b, node->u.list->keys[i]);
            buf_put(b, ":", 1);
            json_node(b, &node->u.list->values[i]);
        }
        buf_put(b, "}", 1);
        break;
    default:  // MPV_FORMAT_NONE, MPV_FORMAT_BYTE_ARRAY
        buf_str(b, "null");
    }
}

static jbyteArray bytes_of(JNIEnv *env, const struct buf *b) {
    jbyteArray array = (*env)->NewByteArray(env, (jsize)b->len);
    if (array)
        (*env)->SetByteArrayRegion(env, array, 0, (jsize)b->len, (const jbyte *)b->data);
    return array;
}

// --- Events ---------------------------------------------------------------------------------------

static void deliver(JNIEnv *env, struct client *c, int id, const char *name, const struct buf *data) {
    jstring jname = name ? (*env)->NewStringUTF(env, name) : NULL;
    jbyteArray jdata = data && data->len ? bytes_of(env, data) : NULL;
    (*env)->CallVoidMethod(env, c->listener, listener_on_event, (jint)id, jname, jdata);
    if ((*env)->ExceptionCheck(env)) {
        (*env)->ExceptionDescribe(env);
        (*env)->ExceptionClear(env);
    }
    if (jname)
        (*env)->DeleteLocalRef(env, jname);
    if (jdata)
        (*env)->DeleteLocalRef(env, jdata);
}

static void log_message(const mpv_event_log_message *msg) {
    int priority = ANDROID_LOG_VERBOSE;
    switch (msg->log_level) {
    case MPV_LOG_LEVEL_FATAL: priority = ANDROID_LOG_FATAL; break;
    case MPV_LOG_LEVEL_ERROR: priority = ANDROID_LOG_ERROR; break;
    case MPV_LOG_LEVEL_WARN: priority = ANDROID_LOG_WARN; break;
    case MPV_LOG_LEVEL_INFO: priority = ANDROID_LOG_INFO; break;
    case MPV_LOG_LEVEL_V: priority = ANDROID_LOG_DEBUG; break;
    default: break;
    }
    // mpv's lines end in a newline, which logcat adds again.
    size_t len = strlen(msg->text);
    if (len && msg->text[len - 1] == '\n')
        len--;
    __android_log_print(priority, TAG, "[%s] %.*s", msg->prefix, (int)len, msg->text);
}

static void *event_thread(void *arg) {
    struct client *c = arg;
    JNIEnv *env = NULL;
    if ((*vm)->AttachCurrentThread(vm, &env, NULL) != JNI_OK) {
        LOGE("could not attach the event thread");
        return NULL;
    }
    struct buf data = {0};
    while (!atomic_load(&c->exiting)) {
        mpv_event *event = mpv_wait_event(c->mpv, -1);
        if (event->event_id == MPV_EVENT_NONE)
            continue;
        data.len = 0;
        switch (event->event_id) {
        case MPV_EVENT_LOG_MESSAGE:
            log_message(event->data);
            break;
        case MPV_EVENT_PROPERTY_CHANGE: {
            mpv_event_property *p = event->data;
            if (p->format == MPV_FORMAT_NODE)
                json_node(&data, p->data);
            else
                buf_str(&data, "null");
            deliver(env, c, event->event_id, p->name, &data);
            break;
        }
        case MPV_EVENT_END_FILE: {
            mpv_event_end_file *end = event->data;
            // The engine's PlayerEnd names; mpv's values skip 1.
            const char *reason = "unknown";
            switch (end->reason) {
            case MPV_END_FILE_REASON_EOF: reason = "eof"; break;
            case MPV_END_FILE_REASON_STOP: reason = "stop"; break;
            case MPV_END_FILE_REASON_QUIT: reason = "quit"; break;
            case MPV_END_FILE_REASON_ERROR: reason = "error"; break;
            case MPV_END_FILE_REASON_REDIRECT: reason = "redirect"; break;
            }
            buf_str(&data, "{\"reason\":");
            json_string(&data, reason);
            if (end->reason == MPV_END_FILE_REASON_ERROR) {
                buf_str(&data, ",\"error\":");
                json_string(&data, mpv_error_string(end->error));
            }
            buf_str(&data, "}");
            deliver(env, c, event->event_id, NULL, &data);
            break;
        }
        case MPV_EVENT_SHUTDOWN:
            deliver(env, c, event->event_id, NULL, NULL);
            atomic_store(&c->exiting, true);
            break;
        default:
            deliver(env, c, event->event_id, mpv_event_name(event->event_id), NULL);
        }
    }
    free(data.data);
    (*vm)->DetachCurrentThread(vm);
    return NULL;
}

// --- Natives --------------------------------------------------------------------------------------

static void n_set_app_context(JNIEnv *env, jclass cls, jobject context) {
    if (app_context)
        return;
    app_context = (*env)->NewGlobalRef(env, context);
    av_jni_set_android_app_ctx(app_context, NULL);
}

static jlong n_create(JNIEnv *env, jclass cls, jobject listener) {
    // mpv parses numbers in the C locale.
    setlocale(LC_NUMERIC, "C");
    mpv_handle *mpv = mpv_create();
    if (!mpv)
        return 0;
    struct client *c = calloc(1, sizeof *c);
    c->mpv = mpv;
    c->listener = (*env)->NewGlobalRef(env, listener);
    atomic_init(&c->exiting, false);
    return (jlong)(intptr_t)c;
}

static jint n_set_option(JNIEnv *env, jclass cls, jlong handle, jstring jname, jstring jvalue) {
    struct client *c = client_of(handle);
    const char *name = (*env)->GetStringUTFChars(env, jname, NULL);
    const char *value = (*env)->GetStringUTFChars(env, jvalue, NULL);
    int r = mpv_set_option_string(c->mpv, name, value);
    if (r < 0)
        LOGW("option %s=%s: %s", name, value, mpv_error_string(r));
    (*env)->ReleaseStringUTFChars(env, jname, name);
    (*env)->ReleaseStringUTFChars(env, jvalue, value);
    return r;
}

static jint n_initialize(JNIEnv *env, jclass cls, jlong handle, jstring jlevel) {
    struct client *c = client_of(handle);
    const char *level = (*env)->GetStringUTFChars(env, jlevel, NULL);
    mpv_request_log_messages(c->mpv, level);
    (*env)->ReleaseStringUTFChars(env, jlevel, level);
    int r = mpv_initialize(c->mpv);
    if (r < 0)
        return r;
    if (pthread_create(&c->thread, NULL, event_thread, c) != 0)
        return MPV_ERROR_GENERIC;
    pthread_setname_np(c->thread, "mpv-events");
    c->thread_started = true;
    return 0;
}

static jint n_command(JNIEnv *env, jclass cls, jlong handle, jobjectArray jargs) {
    struct client *c = client_of(handle);
    jsize n = (*env)->GetArrayLength(env, jargs);
    if (n > 63)
        return MPV_ERROR_INVALID_PARAMETER;
    jstring strings[64] = {0};
    const char *args[64] = {0};
    for (jsize i = 0; i < n; i++) {
        strings[i] = (*env)->GetObjectArrayElement(env, jargs, i);
        args[i] = (*env)->GetStringUTFChars(env, strings[i], NULL);
    }
    int r = mpv_command(c->mpv, args);
    if (r < 0)
        LOGW("command %s: %s", n ? args[0] : "", mpv_error_string(r));
    for (jsize i = 0; i < n; i++) {
        (*env)->ReleaseStringUTFChars(env, strings[i], args[i]);
        (*env)->DeleteLocalRef(env, strings[i]);
    }
    return r;
}

static jint n_set_property(JNIEnv *env, jclass cls, jlong handle, jstring jname, jstring jvalue) {
    struct client *c = client_of(handle);
    const char *name = (*env)->GetStringUTFChars(env, jname, NULL);
    const char *value = (*env)->GetStringUTFChars(env, jvalue, NULL);
    int r = mpv_set_property_string(c->mpv, name, value);
    if (r < 0)
        LOGW("property %s=%s: %s", name, value, mpv_error_string(r));
    (*env)->ReleaseStringUTFChars(env, jname, name);
    (*env)->ReleaseStringUTFChars(env, jvalue, value);
    return r;
}

static jbyteArray n_get_property(JNIEnv *env, jclass cls, jlong handle, jstring jname) {
    struct client *c = client_of(handle);
    const char *name = (*env)->GetStringUTFChars(env, jname, NULL);
    mpv_node node;
    int r = mpv_get_property(c->mpv, name, MPV_FORMAT_NODE, &node);
    (*env)->ReleaseStringUTFChars(env, jname, name);
    if (r < 0)
        return NULL;
    struct buf b = {0};
    json_node(&b, &node);
    mpv_free_node_contents(&node);
    jbyteArray array = bytes_of(env, &b);
    free(b.data);
    return array;
}

static jint n_observe(JNIEnv *env, jclass cls, jlong handle, jstring jname, jlong id) {
    struct client *c = client_of(handle);
    const char *name = (*env)->GetStringUTFChars(env, jname, NULL);
    int r = mpv_observe_property(c->mpv, (uint64_t)id, name, MPV_FORMAT_NODE);
    (*env)->ReleaseStringUTFChars(env, jname, name);
    return r;
}

static jint n_attach_surface(JNIEnv *env, jclass cls, jlong handle, jobject surface) {
    struct client *c = client_of(handle);
    if (c->surface)
        (*env)->DeleteGlobalRef(env, c->surface);
    c->surface = (*env)->NewGlobalRef(env, surface);
    int64_t wid = (int64_t)(intptr_t)c->surface;
    int r = mpv_set_option(c->mpv, "wid", MPV_FORMAT_INT64, &wid);
    if (r < 0)
        LOGE("wid: %s", mpv_error_string(r));
    return r;
}

// The caller has set vo=null first, which mpv carries out before the call returns, so nothing
// draws into the surface by the time its ref goes.
static jint n_detach_surface(JNIEnv *env, jclass cls, jlong handle) {
    struct client *c = client_of(handle);
    int64_t wid = 0;
    int r = mpv_set_option(c->mpv, "wid", MPV_FORMAT_INT64, &wid);
    if (c->surface) {
        (*env)->DeleteGlobalRef(env, c->surface);
        c->surface = NULL;
    }
    return r;
}

// mpv's `screenshot-raw <flags> rgba` as [width, height] in four bytes each, little-endian, then
// the rows, tightly packed. Bitmap.copyPixelsFromBuffer takes the rows as they are.
static jbyteArray n_screenshot(JNIEnv *env, jclass cls, jlong handle, jstring jflags) {
    struct client *c = client_of(handle);
    const char *flags = (*env)->GetStringUTFChars(env, jflags, NULL);
    mpv_node args[3] = {
        {.format = MPV_FORMAT_STRING, .u.string = "screenshot-raw"},
        {.format = MPV_FORMAT_STRING, .u.string = (char *)flags},
        {.format = MPV_FORMAT_STRING, .u.string = "rgba"},
    };
    mpv_node_list list = {.num = 3, .values = args};
    mpv_node cmd = {.format = MPV_FORMAT_NODE_ARRAY, .u.list = &list};
    mpv_node result;
    int r = mpv_command_node(c->mpv, &cmd, &result);
    (*env)->ReleaseStringUTFChars(env, jflags, flags);
    if (r < 0)
        return NULL;
    int64_t w = 0, h = 0, stride = 0;
    struct mpv_byte_array *pixels = NULL;
    if (result.format == MPV_FORMAT_NODE_MAP) {
        for (int i = 0; i < result.u.list->num; i++) {
            const char *key = result.u.list->keys[i];
            mpv_node *v = &result.u.list->values[i];
            if (!strcmp(key, "w") && v->format == MPV_FORMAT_INT64) w = v->u.int64;
            if (!strcmp(key, "h") && v->format == MPV_FORMAT_INT64) h = v->u.int64;
            if (!strcmp(key, "stride") && v->format == MPV_FORMAT_INT64) stride = v->u.int64;
            if (!strcmp(key, "data") && v->format == MPV_FORMAT_BYTE_ARRAY) pixels = v->u.ba;
        }
    }
    jbyteArray array = NULL;
    if (pixels && w > 0 && h > 0 && stride >= w * 4 && (int64_t)pixels->size >= stride * h) {
        jsize row = (jsize)(w * 4);
        array = (*env)->NewByteArray(env, 8 + row * (jsize)h);
        if (array) {
            int32_t header[2] = {(int32_t)w, (int32_t)h};
            (*env)->SetByteArrayRegion(env, array, 0, 8, (const jbyte *)header);
            for (int64_t y = 0; y < h; y++)
                (*env)->SetByteArrayRegion(env, array, 8 + (jsize)y * row, row,
                                           (const jbyte *)pixels->data + y * stride);
        }
    }
    mpv_free_node_contents(&result);
    return array;
}

static void n_destroy(JNIEnv *env, jclass cls, jlong handle) {
    struct client *c = client_of(handle);
    if (!c)
        return;
    if (c->thread_started) {
        atomic_store(&c->exiting, true);
        mpv_wakeup(c->mpv);
        pthread_join(c->thread, NULL);
    }
    mpv_terminate_destroy(c->mpv);
    if (c->surface)
        (*env)->DeleteGlobalRef(env, c->surface);
    (*env)->DeleteGlobalRef(env, c->listener);
    free(c);
}

static const JNINativeMethod methods[] = {
    {"setAppContext", "(Landroid/content/Context;)V", n_set_app_context},
    {"create", "(Ltv/anistream/player/MpvNative$Listener;)J", n_create},
    {"setOption", "(JLjava/lang/String;Ljava/lang/String;)I", n_set_option},
    {"initialize", "(JLjava/lang/String;)I", n_initialize},
    {"command", "(J[Ljava/lang/String;)I", n_command},
    {"setProperty", "(JLjava/lang/String;Ljava/lang/String;)I", n_set_property},
    {"getProperty", "(JLjava/lang/String;)[B", n_get_property},
    {"observe", "(JLjava/lang/String;J)I", n_observe},
    {"attachSurface", "(JLandroid/view/Surface;)I", n_attach_surface},
    {"detachSurface", "(J)I", n_detach_surface},
    {"screenshot", "(JLjava/lang/String;)[B", n_screenshot},
    {"destroy", "(J)V", n_destroy},
};

JNIEXPORT jint JNI_OnLoad(JavaVM *jvm, void *reserved) {
    vm = jvm;
    JNIEnv *env;
    if ((*vm)->GetEnv(vm, (void **)&env, JNI_VERSION_1_6) != JNI_OK)
        return JNI_ERR;
    av_jni_set_java_vm(vm, NULL);
    jclass native = (*env)->FindClass(env, "tv/anistream/player/MpvNative");
    jclass listener = (*env)->FindClass(env, "tv/anistream/player/MpvNative$Listener");
    if (!native || !listener)
        return JNI_ERR;
    listener_on_event = (*env)->GetMethodID(env, listener, "onEvent", "(ILjava/lang/String;[B)V");
    if (!listener_on_event)
        return JNI_ERR;
    if ((*env)->RegisterNatives(env, native, methods, sizeof methods / sizeof methods[0]) != 0)
        return JNI_ERR;
    return JNI_VERSION_1_6;
}
