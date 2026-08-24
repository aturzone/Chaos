plugins {
    id("com.android.application")
    id("org.jetbrains.kotlin.android")
}

android {
    namespace = "com.aturzone.chaos"

    // **34, not the newest.** `compileSdk` has to be a platform that is
    // actually installed, and CI runs on whatever GitHub's image ships. 34 has
    // been on the runner image for a long time; chasing the newest API is how a
    // release build starts failing on a day nobody touched the app.
    compileSdk = 34

    defaultConfig {
        applicationId = "com.aturzone.chaos"
        // **24 (Android 7).** Below that `Theme.Material` behaves differently
        // and TLS defaults change; above it, phones get excluded for nothing.
        // This app is a text box and an HTTP client -- it has no reason to
        // require a recent device.
        minSdk = 24
        targetSdk = 34

        // Kept in step with the Rust workspace by CI, which passes the release
        // tag in. The value here is what a local `gradlew assemble` produces.
        versionCode = 18
        versionName = "0.0.18"
    }

    buildTypes {
        release {
            // **No shrinking.** R8 would strip unused framework code, which is
            // worth doing on an app with dependencies; this one has none, and
            // the APK is already tiny. Turning it on would add a rules file to
            // get wrong and a class of crash that only happens in release.
            isMinifyEnabled = false
        }
    }

    compileOptions {
        sourceCompatibility = JavaVersion.VERSION_17
        targetCompatibility = JavaVersion.VERSION_17
    }

    kotlinOptions {
        jvmTarget = "17"
    }

    // The tree the sources actually live in. `java/` holds Kotlin here, which
    // is conventional and keeps the package path readable.
    sourceSets["main"].java.srcDirs("src/main/java")
}

// **No dependencies in the APK.** Not androidx, not a networking library, not
// a JSON library: `HttpURLConnection` and `org.json` are in the framework, and
// the Rust side of this project has no dependencies either. An APK containing
// only this app is one that cannot break because something else was upgraded.
//
// **JUnit is the one exception and it does not ship.** `ThinkFilter` is a state
// machine over a stream whose tags arrive in fragments -- the kind of thing
// that is wrong in a case nobody thought of, and the kind that cannot be
// checked by looking at a screen. A test-only dependency is a smaller price
// than an untested one of those in shipped code.
dependencies {
    testImplementation("junit:junit:4.13.2")
}
