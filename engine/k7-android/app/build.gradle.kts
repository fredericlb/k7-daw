// The app module. Neither native program is built by AGP's externalNativeBuild: that would configure
// second CMake trees and fetch Tracktion into one. Each `android-arm64` preset builds where every other
// preset does, against the SDK's NDK, and the results are staged here: the engine as the JNI library it
// is, the supervisor as an EXECUTABLE named like one, since only lib*.so files reach the native library
// directory — the one place Android lets an app run its own binary.

plugins {
    id("com.android.application")
    id("org.jetbrains.kotlin.android")
}

// npm is a .cmd on Windows, which an Exec task does not resolve from the bare name.
val npm = if (System.getProperty("os.name").startsWith("Windows")) "npm.cmd" else "npm"

// The table the desktop app starts its backend with too (apps/k7/backend-rs/docs/standalone.md): its port is a constant here.
val standalone = groovy.json.JsonSlurper().parse(rootDir.resolve("../../apps/k7/backend-rs/standalone.json")) as Map<*, *>

// The e2e build's backend port: the shipped app holds standalone.json's on the same loopback, and e2e/src/android.ts dials this one.
val E2E_PORT = 18799

// The root package.json's version, the one every artefact ships under (docs/versioning.md); a snapshot adds its stamp.
val rootVersion = (groovy.json.JsonSlurper().parse(rootDir.resolve("../../package.json")) as Map<*, *>)["version"] as String
val releaseStamp = System.getenv("STUDIO_RELEASE_STAMP").orEmpty()
// versionFor and isStamp of engine/scripts/releaseStamp.mjs, which a change to either rule changes too.
val appVersionName = if (releaseStamp.matches(Regex("snapshot-[0-9a-f]+"))) "$rootVersion-$releaseStamp" else rootVersion

fun git(vararg args: String): String =
    providers.exec { commandLine("git", *args); workingDir = rootDir }.standardOutput.asText.get().trim()

// The commit count: it grows with every descendant commit, so an update always outranks what it replaces.
val appVersionCode = if (git("rev-parse", "--is-shallow-repository") == "true")
    throw GradleException("a shallow clone has no history to count for versionCode (docs/versioning.md)")
else git("rev-list", "--count", "HEAD").toInt()

// The release key comes from the environment only, never from a file in the tree (docs/release.md).
val releaseKeystore = System.getenv("K7_ANDROID_KEYSTORE_FILE")?.takeIf { it.isNotBlank() }

fun releaseSecret(name: String): String =
    System.getenv(name)?.takeIf { it.isNotEmpty() } ?: throw GradleException("K7_ANDROID_KEYSTORE_FILE is set but $name is not")

android {
    namespace = "se.frediver.k7"
    compileSdk = 36
    ndkVersion = "29.0.14206865"

    defaultConfig {
        applicationId = "se.frediver.k7.daw"
        minSdk = 29
        targetSdk = 36
        versionCode = appVersionCode
        versionName = appVersionName
        ndk { abiFilters += "arm64-v8a" }
        buildConfigField("int", "STANDALONE_PORT", standalone["port"].toString())
        // The backend's folders the engine is sent files from by their own paths: its workspaces and médiathèque.
        val dirs = standalone["dirs"] as Map<*, *>
        buildConfigField("String", "ENGINE_ROOT_DIRS", "\"${dirs["STUDIO_FILES_ROOT"]};${dirs["STUDIO_MEDIATEK"]}\"")
        manifestPlaceholders["appLabel"] = "K7 DAW"
    }

    buildFeatures { buildConfig = true }

    signingConfigs {
        if (releaseKeystore != null) {
            create("release") {
                storeFile = file(releaseKeystore)
                storePassword = releaseSecret("K7_ANDROID_KEYSTORE_PASSWORD")
                keyAlias = releaseSecret("K7_ANDROID_KEY_ALIAS")
                keyPassword = releaseSecret("K7_ANDROID_KEY_PASSWORD")
            }
        }
    }

    // Without the key's environment, assembleRelease leaves app-release-unsigned.apk, which nothing publishes.
    buildTypes {
        getByName("release") {
            isDebuggable = false
            // No R8: Capacitor's bridge and the JNI entry points are reached by reflection, and no keep rules cover them.
            isMinifyEnabled = false
            isShrinkResources = false
            signingConfig = signingConfigs.findByName("release")
        }
        // The functional suite's build: installed BESIDE the shipped app, so its own id, port and label (e2e/docs/e2e.md).
        create("e2e") {
            initWith(getByName("debug"))
            applicationIdSuffix = ".e2e"
            versionNameSuffix = "-e2e"
            buildConfigField("int", "STANDALONE_PORT", E2E_PORT.toString())
            manifestPlaceholders["appLabel"] = "K7 DAW e2e"
            matchingFallbacks += "debug"
        }
    }

    // 21: Capacitor's runtime is built for it, and the app's own Java and Kotlin must agree with each other.
    compileOptions {
        sourceCompatibility = JavaVersion.VERSION_21
        targetCompatibility = JavaVersion.VERSION_21
    }

    sourceSets["main"].jniLibs.srcDir(layout.buildDirectory.dir("engine-jni"))

    // Extracted to disk at install rather than mapped from the APK: an executable must be a real file.
    packaging { jniLibs { useLegacyPackaging = true } }
}

kotlin {
    compilerOptions { jvmTarget.set(org.jetbrains.kotlin.gradle.dsl.JvmTarget.JVM_21) }
}

// The versions Capacitor's own module is built against (its build.gradle's defaults), for what the app
// uses of them directly: BridgeActivity is an AppCompatActivity, AppViewsPlugin draws under the insets, and
// StudioBridge talks to the apps' pages through androidx.webkit's WebMessageListener.
dependencies {
    implementation(project(":capacitor-android"))
    implementation("androidx.appcompat:appcompat:1.7.1")
    implementation("androidx.activity:activity:1.11.0")
    implementation("androidx.core:core:1.17.0")
    implementation("androidx.coordinatorlayout:coordinatorlayout:1.3.0")
    implementation("androidx.webkit:webkit:1.14.0")
    // CertTrust's decision runs on the JVM alone (app/src/test).
    testImplementation("junit:junit:4.13.2")
}

val engineDir = rootDir.resolve("../k7-runtime")
val supervisorDir = rootDir.resolve("../k7-supervisor")

// The preset, with this SDK's NDK, through the same script as build:mac and build:win.
val buildEngine by tasks.registering(Exec::class) {
    workingDir = engineDir
    environment("ANDROID_NDK_HOME", android.ndkDirectory.absolutePath)
    commandLine("node", "scripts/build.mjs", "android-arm64")
}

// The supervisor through engine/scripts/supervisorBuild.mjs, the desktop build's own entry, told which preset to use.
val buildSupervisor by tasks.registering(Exec::class) {
    workingDir = rootDir.resolve("../..")
    environment("ANDROID_NDK_HOME", android.ndkDirectory.absolutePath)
    environment("STUDIO_SUPERVISOR_PRESET", "android-arm64")
    commandLine(
        "node", "-e",
        "import('./engine/scripts/supervisorBuild.mjs').then((m) => m.buildSupervisor(process.cwd()))",
    )
}

val stageEngine by tasks.registering(Copy::class) {
    dependsOn(buildEngine, buildSupervisor)
    from(engineDir.resolve("build/android-arm64/libk7runtime.so"))
    from(supervisorDir.resolve("build/android-arm64/k7-supervisor")) { rename { "libk7supervisor.so" } }
    into(layout.buildDirectory.dir("engine-jni/arm64-v8a"))
}

android.sourceSets["main"].assets.srcDir(layout.buildDirectory.dir("engine-assets"))

// The page (web/: the engine tray, shown while no studio is connected), built by its workspace and staged the way `cap copy` would
// lay it out: the bundle under public/, beside the config Capacitor reads at start and the list of npm
// plugins it would load — none, since the app's two plugins are registered by MainActivity itself.
val webDir = rootDir.resolve("web")
val shellOut = layout.buildDirectory.dir("shell-assets")

val buildShell by tasks.registering(Exec::class) {
    workingDir = webDir
    inputs.files(fileTree(webDir.resolve("src")), webDir.resolve("index.html"), webDir.resolve("vite.config.ts"), webDir.resolve("package.json"))
    outputs.dir(webDir.resolve("dist"))
    commandLine(npm, "run", "build")
}

val stageShell by tasks.registering(Sync::class) {
    dependsOn(buildShell)
    from(webDir.resolve("dist")) { into("public") }
    from(webDir.resolve("capacitor.config.json"))
    from(resources.text.fromString("[]")) { rename { "capacitor.plugins.json" } }
    into(shellOut)
}

android.sourceSets["main"].assets.srcDir(shellOut)

val repoDir = rootDir.resolve("../..")

// The frontend the front serves, as the desktop app's staging builds it.
val buildFrontend by tasks.registering(Exec::class) {
    workingDir = repoDir
    commandLine(npm, "exec", "--workspace", "k7-frontend", "--", "vite", "build")
}

val backendTree = layout.buildDirectory.dir("backend-tree")

val bundleBackend by tasks.registering(Exec::class) {
    dependsOn(buildFrontend)
    workingDir = repoDir
    commandLine("node", repoDir.resolve("apps/k7/backend-rs/scripts/stageBundle.mjs").absolutePath, backendTree.get().asFile.absolutePath)
}

// The files the Rust front's actor reads at its code root, unpacked as `code/` and named K7_CODE_ROOT (docs/android.md).
val codeRootTree = layout.buildDirectory.dir("code-root")

val stageCodeRoot by tasks.registering(Exec::class) {
    workingDir = repoDir
    commandLine("node", repoDir.resolve("apps/k7/backend-rs/scripts/stageCodeRoot.mjs").absolutePath, codeRootTree.get().asFile.absolutePath)
}

val backendAssets = layout.buildDirectory.dir("backend-assets")

val stageBackend by tasks.registering(Zip::class) {
    dependsOn(bundleBackend, stageCodeRoot)
    from(backendTree)
    from(codeRootTree) { into("code") }
    from(repoDir.resolve("apps/k7/backend-rs/standalone.json"))
    archiveFileName.set("backend.zip")
    destinationDirectory.set(backendAssets)
}

android.sourceSets["main"].assets.srcDir(backendAssets)
// The zip is already compressed: aapt must not deflate it again, and the app streams it out as it is.
android.androidResources.noCompress += "zip"

tasks.named("preBuild") { dependsOn(stageEngine, stageShell, stageBackend) }

// The Rust front as libk7_embed.so, cargo linking with this SDK's NDK at the app's minSdk (apps/k7/backend-rs/docs/embed.md).
val frontDir = repoDir.resolve("apps/k7/backend-rs")
val frontJni = layout.buildDirectory.dir("front-jni")

// The front's cargo target lives outside the tree, so an APK build leaves no backend-rs/target behind (docs/rust-builds.md).
val frontTarget = File(System.getProperty("user.home"), ".cache/k7-android-front")

val buildFront by tasks.registering(Exec::class) {
    workingDir = frontDir
    val os = System.getProperty("os.name").lowercase()
    val host = when {
        os.contains("windows") -> "windows-x86_64"
        os.contains("mac") -> "darwin-x86_64"
        else -> "linux-x86_64"
    }
    val suffix = if (os.contains("windows")) ".cmd" else ""
    val clang = android.ndkDirectory.resolve("toolchains/llvm/prebuilt/$host/bin/aarch64-linux-android${android.defaultConfig.minSdk}-clang$suffix")
    environment("CARGO_TARGET_AARCH64_LINUX_ANDROID_LINKER", clang.absolutePath)
    // rusqlite compiles SQLite's C: the cc crate needs the same NDK compiler and archiver.
    environment("CC_aarch64_linux_android", clang.absolutePath)
    environment("AR_aarch64_linux_android", android.ndkDirectory.resolve("toolchains/llvm/prebuilt/$host/bin/llvm-ar${if (os.contains("windows")) ".exe" else ""}").absolutePath)
    // rustc through sccache when the machine has it, unless RUSTC_WRAPPER is set already (docs/rust-builds.md).
    providers.exec { commandLine("node", repoDir.resolve("engine/scripts/sccache.mjs").absolutePath, "dev") }
        .standardOutput.asText.get().lines().filter { it.contains('=') }
        .forEach { environment(it.substringBefore('='), it.substringAfter('=')) }
    commandLine("cargo", "build", "--release", "--target-dir", frontTarget.absolutePath, "-p", "k7-embed", "--target", "aarch64-linux-android")
}

val stageFront by tasks.registering(Copy::class) {
    dependsOn(buildFront)
    from(frontTarget.resolve("aarch64-linux-android/release/libk7_embed.so"))
    into(frontJni.get().dir("arm64-v8a"))
}

android.sourceSets["main"].jniLibs.srcDir(frontJni)
tasks.named("preBuild") { dependsOn(stageFront) }
