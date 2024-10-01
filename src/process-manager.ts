export class ProcessManager {
	childProcess: Deno.ChildProcess;

	// no piping of stdout and stderr because Arma Reforger Server behaves strangely
	// later we will read the logs created directly by Arma Reforger Server
	constructor(cwd: string, executeable: string, args: string[]) {
		// preparing arguments for shell command
		const shArgs = ['-c', `./${executeable} ${args.join(' ')}`];

		// starting Arma Reforger Server within a shell because otherwise behaves strangely
		const command = new Deno.Command('sh', {
			cwd: cwd,
			args: shArgs,
			stdout: 'null',
			stderr: 'null',
		});

		// spawning the process
		this.childProcess = command.spawn();

		this.childProcess.status.then((status) =>
			console.log(
				`Launched process finished with success status: ${status.success}`,
			)
		);
	}
}
