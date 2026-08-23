// The Android client is deliberately NOT part of the Cargo workspace: it is not
// a Rust crate, it builds with a different toolchain, and `cargo test` must not
// need a JDK. It has its own Gradle build here and its own CI job.
pluginManagement {
    repositories {
        google()
        mavenCentral()
        gradlePluginPortal()
    }
}
dependencyResolutionManagement {
    repositoriesMode.set(RepositoriesMode.FAIL_ON_PROJECT_REPOS)
    repositories {
        google()
        mavenCentral()
    }
}

rootProject.name = "chaos-android"
include(":app")
