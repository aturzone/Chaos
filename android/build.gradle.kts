// Versions in one place, and pinned. An Android build that floats its plugin
// version is one that breaks on a day nobody changed anything.
plugins {
    id("com.android.application") version "8.5.2" apply false
    id("org.jetbrains.kotlin.android") version "1.9.24" apply false
}
