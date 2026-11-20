pluginManagement {
  repositories {
    google {
      content {
        includeGroupByRegex("androidx.*")
        includeGroupByRegex("com\\.android.*")
        includeGroupByRegex("com\\.google.*")
      }
    }
    mavenCentral()
    gradlePluginPortal()
    // Kopia Maven Central (Google) – gdy repo.maven.apache.org chwilowo nie odpowiada.
    maven("https://maven-central.storage-download.googleapis.com/maven2/")
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
    mavenCentral()
    maven("https://maven-central.storage-download.googleapis.com/maven2/")
  }
}

rootProject.name = "AsOne"

include(":app")
