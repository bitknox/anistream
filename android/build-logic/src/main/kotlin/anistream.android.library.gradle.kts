// A library module of the app: Android 15 and up, built against the newest SDK.
plugins {
    id("com.android.library")
}

android {
    compileSdk = AnistreamSdk.COMPILE
    defaultConfig {
        minSdk = AnistreamSdk.MIN
    }
    compileOptions {
        sourceCompatibility = AnistreamSdk.JAVA
        targetCompatibility = AnistreamSdk.JAVA
    }
    testOptions {
        // Android's own classes answer with their defaults in a unit test, rather than throwing.
        unitTests.isReturnDefaultValues = true
    }
}
