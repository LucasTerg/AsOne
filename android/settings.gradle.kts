pluginManagement {
  repositories {
    google {
      content {
        includeGroupByRegex("androidx.*")
        includeGroupByRegex("com\\.android.*")
        includeGroupByRegex("com\\.google.*")
      }
    }
    // Kopia Maven Central (Google) PRZED repo.maven.apache.org: Gradle przy błędzie repozytorium
    // (np. 429 Too Many Requests dla botów CI) nie próbuje następnego – szuka dalej tylko przy 404.
    maven("https://maven-central.storage-download.googleapis.com/maven2/")
    mavenCentral()
    gradlePluginPortal()
  }
}

dependencyResolutionManagement {
  repositoriesMode.set(RepositoriesMode.FAIL_ON_PROJECT_REPOS)
  repositories {
    google {
      content {
        includeGroupByRegex("androidx.*")
        includeGroupByRegex("com\\.android.*")
        includeGroupByRegex("com\\.google.*")
      }
    }
    maven("https://maven-central.storage-download.googleapis.com/maven2/")
    mavenCentral()
  }
}

rootProject.name = "AsOne"

include(":app")
