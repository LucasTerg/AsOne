// AsOne na Androida: powłoka Kotlin (GameActivity) + natywna biblioteka libasone_app.so z Rusta.
// Biblioteki .so buduje `cargo ndk` do app/src/main/jniLibs (patrz README › Android).
plugins {
  alias(libs.plugins.android.application)
}

// Wersja z Cargo.toml (workspace) – jedna dla wszystkich platform.
// versionCode jak w cargo-apk (1 << 24 | major << 16 | minor << 8 | patch): aktualizacje
// z wcześniejszych APK (0.1.0–0.1.3) działają dalej.
val cargoVersion: Triple<Int, Int, Int> = run {
  val text = rootProject.file("../Cargo.toml").readText()
  val m = Regex("""(?m)^version\s*=\s*"(\d+)\.(\d+)\.(\d+)"""").find(text)
    ?: error("Brak wersji w Cargo.toml")
  Triple(m.groupValues[1].toInt(), m.groupValues[2].toInt(), m.groupValues[3].toInt())
}

// Podpis wydania: magazyn PKCS12 ze zmiennych środowiskowych (CI: sekrety ANDROID_KEYSTORE_*).
// Bez nich wydanie podpisuje klucz debug (do testów).
val releaseKeystore: String? = System.getenv("ASONE_KEYSTORE")

android {
  namespace = "io.github.lucasterg.asone"
  compileSdk = 36

  defaultConfig {
    applicationId = "io.github.lucasterg.asone"
    minSdk = 26
    targetSdk = 36
    val (major, minor, patch) = cargoVersion
    versionCode = (1 shl 24) or (major shl 16) or (minor shl 8) or patch
    versionName = "$major.$minor.$patch"
    ndk {
      abiFilters.addAll(listOf("arm64-v8a", "x86_64"))
    }
  }

  signingConfigs {
    create("release") {
      if (releaseKeystore != null) {
        storeFile = file(releaseKeystore)
        storeType = "pkcs12"
        storePassword = System.getenv("ASONE_KEYSTORE_PASSWORD")
        keyAlias = System.getenv("ASONE_KEY_ALIAS") ?: "asone"
        keyPassword = System.getenv("ASONE_KEYSTORE_PASSWORD")
      }
    }
  }

  buildTypes {
    release {
      // Bez R8: metody aktywności wołane z Rusta przez JNI muszą zostać.
      isMinifyEnabled = false
      signingConfig = if (releaseKeystore != null) signingConfigs.getByName("release") else signingConfigs.getByName("debug")
    }
  }

  compileOptions {
    sourceCompatibility = JavaVersion.VERSION_17
    targetCompatibility = JavaVersion.VERSION_17
  }

  packaging {
    jniLibs {
      // Skompresowane .so w APK: mniejsze pobieranie (biblioteki ~20 MB na architekturę).
      useLegacyPackaging = true
    }
  }
}

dependencies {
  implementation(libs.androidx.core.ktx)
  implementation(libs.androidx.appcompat)
  implementation(libs.androidx.games.activity)
}
