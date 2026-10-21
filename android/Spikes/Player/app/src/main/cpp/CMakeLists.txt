# libanistream_mpv.so, the JNI shim, against the libmpv and FFmpeg android/scripts/mpv.sh built.
cmake_minimum_required(VERSION 3.22)
project(anistream_mpv C)

set(MPV_DIR "${CMAKE_CURRENT_SOURCE_DIR}/../../../../../../.mpv/current" CACHE PATH "android/.mpv/current")
if(NOT EXISTS "${MPV_DIR}/jniLibs/${ANDROID_ABI}/libmpv.so")
  message(FATAL_ERROR "No libmpv in ${MPV_DIR}: run android/scripts/mpv.sh")
endif()

foreach(lib mpv avcodec)
  add_library(${lib} SHARED IMPORTED)
  set_target_properties(${lib} PROPERTIES IMPORTED_LOCATION "${MPV_DIR}/jniLibs/${ANDROID_ABI}/lib${lib}.so")
endforeach()

add_library(anistream_mpv SHARED anistream_mpv.c)
target_include_directories(anistream_mpv PRIVATE "${MPV_DIR}/include")
target_compile_options(anistream_mpv PRIVATE -Wall -Wextra -Wno-unused-parameter -Werror)
target_link_libraries(anistream_mpv mpv avcodec log)
target_link_options(anistream_mpv PRIVATE "-Wl,-z,max-page-size=16384")
