plugins {
    id("com.android.application")
    id("org.jetbrains.kotlin.android")
}

android {
    // `Engine.version()` reads BuildConfig.VERSION_NAME, so the version the
    // app reports is the one the build stamped rather than a second constant
    // that can drift from it.
    buildFeatures { buildConfig = true }

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
        versionCode = 21
        versionName = "0.0.21"
    }

    // **A stable identity, or Android refuses to upgrade in place.**
    //
    // v0.0.31 could not be installed over v0.0.30: "App not installed". The
    // `versionCode` was incrementing correctly, so the only remaining cause was
    // the signature -- and the release built with `assembleDebug` on a *fresh CI
    // runner*, where `~/.android/debug.keystore` does not exist and Gradle
    // generates a new one, with a new random key, on every run.
    //
    // The workflow's own comment predicted exactly this ("generating one per run
    // would give every release a different identity and Android would refuse to
    // upgrade in place") and then relied on the debug key being "a key everyone
    // has". That is true on a developer's machine, where the keystore persists.
    // It is false on an ephemeral runner.
    //
    // So: sign with a keystore supplied by the build when there is one. CI
    // decodes it from a repository secret. With no keystore the build falls back
    // to the debug key, which still installs and still cannot be upgraded over
    // -- and the release notes have to say so rather than let someone discover
    // it with a phone in their hand.
    // **An absent secret arrives as an EMPTY STRING, not as null**, because the
    // workflow sets these from `secrets.*` unconditionally and an undefined
    // secret expands to nothing. The first version of this block read
    // `System.getenv(...)` straight into a `String?` and got `""`, which is not
    // null, so the guard below called `file("")` and every release APK build
    // died at configuration time with `Cannot convert '' to File` -- the exact
    // case this code exists to handle gracefully. It shipped that way because
    // the keystore support landed after v0.0.31 was tagged and no release build
    // ran it until v0.0.32.
    fun setting(property: String, variable: String): String? =
        (providers.gradleProperty(property).orNull ?: System.getenv(variable))
            ?.takeIf { it.isNotBlank() }

    val keystorePath = setting("chaos.keystore", "CHAOS_KEYSTORE")
    val keystorePass = setting("chaos.keystore.password", "CHAOS_KEYSTORE_PASSWORD")
    val keyAliasName = setting("chaos.key.alias", "CHAOS_KEY_ALIAS")
    val keyPass = setting("chaos.key.password", "CHAOS_KEY_PASSWORD")

    signingConfigs {
        if (keystorePath != null && file(keystorePath).exists()) {
            create("chaos") {
                storeFile = file(keystorePath)
                storePassword = keystorePass
                keyAlias = keyAliasName
                keyPassword = keyPass
            }
        }
    }

    buildTypes {
        release {
            // **No shrinking.** R8 would strip unused framework code, which is
            // worth doing on an app with dependencies; this one has none, and
            // the APK is already tiny. Turning it on would add a rules file to
            // get wrong and a class of crash that only happens in release.
            isMinifyEnabled = false
            signingConfig = signingConfigs.findByName("chaos")
        }
        // **Debug too**, because that is what ships today. A release APK signed
        // with one identity and a debug APK signed with another would still
        // refuse to upgrade across the two, which is the same bug wearing a
        // different hat.
        getByName("debug") {
            signingConfigs.findByName("chaos")?.let { signingConfig = it }
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
