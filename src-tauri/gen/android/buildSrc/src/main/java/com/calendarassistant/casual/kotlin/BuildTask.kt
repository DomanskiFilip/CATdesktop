import java.io.File
import org.apache.tools.ant.taskdefs.condition.Os
import org.gradle.api.DefaultTask
import org.gradle.api.GradleException
import org.gradle.api.logging.LogLevel
import org.gradle.api.tasks.Input
import org.gradle.api.tasks.TaskAction

open class BuildTask : DefaultTask() {
    @Input
    var rootDirRel: String? = null
    @Input
    var target: String? = null
    @Input
    var release: Boolean? = null

    @TaskAction
    fun assemble() {
        // The executable should be 'cargo'
        val cargoExecutable = if (Os.isFamily(Os.FAMILY_WINDOWS)) "cargo.exe" else "cargo"
        runTauriCli(cargoExecutable)
    }

    fun runTauriCli(executable: String) {
        val rootDirRel = rootDirRel ?: throw GradleException("rootDirRel cannot be null")
        val target = target ?: throw GradleException("target cannot be null")
        val release = release ?: throw GradleException("release cannot be null")

        // Arguments for 'cargo tauri'
        val args = mutableListOf(
            "tauri",
            "android",
            "android-studio-script",
            "--target", target
        )

        if (release) {
            args.add("--release")
        }

        if (project.logger.isEnabled(LogLevel.DEBUG)) {
            args.add("-vv")
        } else if (project.logger.isEnabled(LogLevel.INFO)) {
            args.add("-v")
        }

        project.exec {
            workingDir(File(project.projectDir, rootDirRel))
            executable(executable) // This will be "cargo"
            args(args)
        }.assertNormalExitValue()
    }
}