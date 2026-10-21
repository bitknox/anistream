// The app module: Android 15 and up, built against and targeting the newest SDK.
plugins {
    id("com.android.application")
}

android {
    compileSdk = AnistreamSdk.COMPILE
    defaultConfig {
        minSdk = AnistreamSdk.MIN
        targetSdk = AnistreamSdk.COMPILE
    }
    compileOptions {
        sourceCompatibility = AnistreamSdk.JAVA
        targetCompatibility = AnistreamSdk.JAVA
    }
}
