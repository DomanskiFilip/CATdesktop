buildscript {
    repositories {
        google()
        mavenCentral()
    }
    dependencies {
        classpath("com.android.tools.build:gradle:8.11.0")
        classpath("org.jetbrains.kotlin:kotlin-gradle-plugin:1.9.25")
    }
}

allprojects {
    repositories {
        google()
        mavenCentral()
    }
}

subprojects {
    afterEvaluate {
        // Only apply to the app module if it exists
        if (project.name == "app") {
            extensions.findByName("android")?.let { ext ->
                (ext as com.android.build.gradle.BaseExtension).apply {
                    compileSdkVersion(35)
                    defaultConfig {
                        minSdkVersion(28)
                        targetSdkVersion(35)
                    }
                }
            }
        }
    }
}

tasks.register("clean").configure {
    delete("build")
}

