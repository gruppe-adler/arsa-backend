import { join } from '@std/path';

import type {
	DockerStats,
	IsRunningUpdate,
	ResultSize,
	Server,
	ServerConfig,
	ServerStatusUpdate,
} from './interfaces.ts';
import { defaultServer } from './defaults.ts';

export class ArmaReforgerServer {
	uuid: string;
	serverPath: string;
	configPath: string;
	arsContainerId: string;
	isRunning: boolean;
	messageQueue: ServerStatusUpdate[];
	checkInterval: number;

	/* ---------------------------------------- */

	constructor(uuid: string) {
		if (!uuid) throw new Error('UUID is missing.');

		this.uuid = uuid;
		this.serverPath = join(Deno.cwd(), 'servers', this.uuid, 'server.json');
		this.configPath = join(Deno.cwd(), 'servers', this.uuid, 'config.json');
		this.messageQueue = [];
		this.arsContainerId = '';
		this.isRunning = false;
		this.checkInterval = 0;

		this.isRunning = this.checkIsRunning();
		this.setIsRunning(this.isRunning);

		this.startIsStillRunningChecker();
	}

	/* ---------------------------------------- */

	startIsStillRunningChecker(): void {
		this.checkInterval = setInterval(() => {
			if (!this.checkIsRunning()) {
				this.isRunning = false;
				this.setIsRunning(this.isRunning);
				this.messageQueue.push({
					type: 'isRunningUpdate',
					uuid: this.uuid,
					isRunning: this.isRunning,
				} as IsRunningUpdate);
				clearInterval(this.checkInterval);
			}
		}, 1_000);
	}

	/* ---------------------------------------- */

	getStartupParametersArgsArray(): string[] {
		let server: Server = defaultServer;
		try {
			server = JSON.parse(Deno.readTextFileSync(this.serverPath));
		} catch (error) {
			console.log(error);
			return [];
		}

		const args: string[] = [];

		server.startupParameters.forEach((startupParameter) => {
			if (startupParameter.enabled) {
				args.push(`-${startupParameter.parameter}`);
				if (startupParameter.value) {
					args.push(startupParameter.value as string);
				}
			}
		});

		return args;
	}

	/* ---------------------------------------- */

	start(): void {
		// read config to allow accessing values for command
		const decoder = new TextDecoder('utf-8');
		const fileContent = Deno.readFileSync(this.configPath);
		const config: ServerConfig = JSON.parse(decoder.decode(fileContent));

		const network = Deno.env.get('NETWORK') || 'default';
		const mountTypeProfiles = Deno.env.get('MOUNT_TYPE_PROFILES') || 'bind';
		const mountTypeServers = Deno.env.get('MOUNT_TYPE_SERVERS') || 'bind';
		const sourceProfiles = Deno.env.get('SOURCE_PROFILES') ||
			'/etc/arsa/arsa-profiles';
		const sourceServers = Deno.env.get('SOURCE_SERVERS') ||
			'/etc/arsa/arsa-servers';

		// starting Arma Reforger Server within a Docker Container
		// it's important to NOT combine multiple arguments like '-p' and '2001' in one argument '-p 2001'
		// otherwise it results in additional white spaces that break the call of docker with
		// '-p 2001: 2001' instead of '-p 2001:2001'

		const args = [
			'run',
			'-d',
			'--rm',
			`--network=${network}`,
			'--hostname',
			this.uuid,
			'--mount',
			`type=${mountTypeProfiles},source=${sourceProfiles},target=/ars/profiles`,
			'--mount',
			`type=${mountTypeServers},source=${sourceServers},target=/ars/servers,readonly`,
			'-p',
			`${config.bindPort}:${config.bindPort}/udp`,
			'-p',
			`${config.a2s.port}:${config.a2s.port}/udp`,
			'-p',
			`${config.rcon.port}:${config.rcon.port}/udp`,
			'--name',
			this.uuid,
			'ars',
			'-config',
			`/ars/servers/${this.uuid}/config.json`,
			'-profile',
			`/ars/profiles/${this.uuid}`,
		];

		const command = new Deno.Command('docker', {
			cwd: join(Deno.cwd(), 'ars'),
			args: args.concat(this.getStartupParametersArgsArray()),
		});

		const { code, stdout, stderr } = command.outputSync();
		const output = (new TextDecoder().decode(stdout)).slice(0, -1);

		// if code = 0 then the first 64 characters on stdout should be the container id
		console.log(`docker run exit code: ${code}`);
		if (code === 0) {
			this.isRunning = true;
			this.arsContainerId = output.substring(0, 64);
			console.log(`SUCCESS: ${output}`);
		} else {
			this.isRunning = false;
			const error = (new TextDecoder().decode(stderr)).slice(0, -1);
			console.log(`ERROR: ${error}`);
		}

		this.setIsRunning(this.isRunning);

		this.messageQueue.push({
			type: 'isRunningUpdate',
			uuid: this.uuid,
			isRunning: this.isRunning,
		} as IsRunningUpdate);

		console.log('Arma Reforger Server started.');

		this.startIsStillRunningChecker();
	}

	/* ---------------------------------------- */

	stop(): void {
		const command = new Deno.Command('docker', {
			cwd: join(Deno.cwd(), 'ars'),
			args: [
				'kill',
				this.uuid,
			],
		});

		const { code, stdout, stderr } = command.outputSync();
		const output = (new TextDecoder().decode(stdout)).slice(0, -1);

		// if code = 0 then reset container id
		console.log(`docker kill exit code: ${code}`);
		if (code === 0) {
			this.isRunning = false;
			this.arsContainerId = '';
			console.log(`SUCCESS: ${output}`);
		} else {
			const error = (new TextDecoder().decode(stderr)).slice(0, -1);
			console.log(`ERROR: ${error}`);
		}

		this.setIsRunning(this.isRunning);

		this.messageQueue.push({
			type: 'isRunningUpdate',
			uuid: this.uuid,
			isRunning: this.isRunning,
		} as IsRunningUpdate);

		console.log('Arma Refoger Server stopped.');

		clearInterval(this.checkInterval);
	}

	/* ---------------------------------------- */

	checkIsRunning(): boolean {
		const command = new Deno.Command('docker', {
			cwd: join(Deno.cwd(), 'ars'),
			args: [
				'container',
				'inspect',
				'-f',
				'{{.State.Running}}',
				this.uuid,
			],
		});

		const { code, stdout, stderr } = command.outputSync();

		const output = (new TextDecoder().decode(stdout)).slice(0, -1);

		if (code === 0 && output.substring(0, 4) === 'true') {
			//console.log(`SUCCESS: ${output}`);
			return true;
		} else {
			const error = (new TextDecoder().decode(stderr)).slice(0, -1);
			console.log(`ERROR(RETURN CODE ${code}): ${error}`);
			return false;
		}
	}

	/* ---------------------------------------- */

	setIsRunning(isRunning: boolean) {
		try {
			const server: Server = JSON.parse(
				Deno.readTextFileSync(this.serverPath),
			);
			server.isRunning = isRunning;
			Deno.writeTextFileSync(
				this.serverPath,
				JSON.stringify(server, null, 2),
			);
		} catch (error) {
			console.log(error);
		}
	}

	/* ---------------------------------------- */

	async getStats(): Promise<DockerStats | null> {
		// docker stats 0cb65489-efd4-4e2e-b495-06d5b4f213ed --no-stream --format "{{ json . }}"
		// example output
		/* 		{
			"BlockIO":"0B / 0B",
			"CPUPerc":"29.74%",
			"Container":"0cb65489-efd4-4e2e-b495-06d5b4f213ed",
			"ID":"d76cd514a9e3",
			"MemPerc":"19.82%",
			"MemUsage":"3.017GiB / 15.22GiB",
			"Name":"0cb65489-efd4-4e2e-b495-06d5b4f213ed",
			"NetIO":"26.3kB / 11.1kB",
			"PIDs":"31"
		} */

		const command = new Deno.Command('docker', {
			cwd: join(Deno.cwd(), 'ars'),
			args: [
				'stats',
				this.uuid,
				'--no-stream',
				'--format',
				'{{ json . }}',
			],
		});

		let stats: DockerStats | null = null;

		const { code, stdout, stderr } = await command.output();
		if (code === 0) {
			stats = JSON.parse(new TextDecoder().decode(stdout));
		}

		return stats;
	}

	/* ---------------------------------------- */

	async getSize(): Promise<ResultSize | null> {
		const result: ResultSize = {
			profileDir: '0B',
			serverDir: '0B',
			modsDir: '0B',
			logsDir: '0B',
			allMods: '',
			allLogs: '',
		};

		/* ---------- */

		const commandServerDir = new Deno.Command('du', {
			cwd: join(Deno.cwd(), 'servers'),
			args: [
				'-sh',
				this.uuid,
			],
		});

		const commandServerDirOutput = await commandServerDir.output();
		if (commandServerDirOutput.success) {
			result.serverDir =
				(new TextDecoder().decode(commandServerDirOutput.stdout)).split(
					'\t',
				)[0];
		}

		/* ---------- */

		try {
			await Deno.stat(join(Deno.cwd(), 'profiles', this.uuid));
		} catch (err) {
			if (!(err instanceof Deno.errors.NotFound)) {
				throw err;
			}
			return result;
		}

		/* ---------- */

		const commandProfileDir = new Deno.Command('du', {
			cwd: join(Deno.cwd(), 'profiles'),
			args: [
				'-sh',
				this.uuid,
			],
		});

		const commandProfileDirOutput = await commandProfileDir.output();
		if (commandProfileDirOutput.success) {
			result.profileDir =
				(new TextDecoder().decode(commandProfileDirOutput.stdout))
					.split('\t')[0];
		}

		/* ---------- */

		const commandModsDir = new Deno.Command('du', {
			cwd: join(Deno.cwd(), 'profiles'),
			args: [
				'-sh',
				join(this.uuid, 'addons'),
			],
		});

		const commandModsDirOutput = await commandModsDir.output();
		if (commandModsDirOutput.success) {
			result.modsDir =
				(new TextDecoder().decode(commandModsDirOutput.stdout)).split(
					'\t',
				)[0];
		}

		/* ---------- */

		const commandLogsDir = new Deno.Command('du', {
			cwd: join(Deno.cwd(), 'profiles'),
			args: [
				'-sh',
				join(this.uuid, 'logs'),
			],
		});

		const commandLogsDirOutput = await commandLogsDir.output();
		if (commandLogsDirOutput.success) {
			result.logsDir =
				(new TextDecoder().decode(commandLogsDirOutput.stdout)).split(
					'\t',
				)[0];
		}

		/* ---------- */

		const commandSingleModsDir = new Deno.Command('du', {
			cwd: join(Deno.cwd(), 'profiles', this.uuid, 'addons'),
			args: [
				'-h',
				'-d',
				'1',
			],
		});

		const commandSingleModsDirOutput = await commandSingleModsDir.output();
		if (commandSingleModsDirOutput.success) {
			result.allMods = new TextDecoder().decode(
				commandSingleModsDirOutput.stdout,
			);
		}

		/* ---------- */

		const commandSingleLogsDir = new Deno.Command('du', {
			cwd: join(Deno.cwd(), 'profiles', this.uuid, 'logs'),
			args: [
				'-h',
				'-d',
				'1',
			],
		});

		const commandSingleLogsDirOutput = await commandSingleLogsDir.output();
		if (commandSingleLogsDirOutput.success) {
			result.allLogs = new TextDecoder().decode(
				commandSingleLogsDirOutput.stdout,
			);
		}

		/* ---------- */

		return result;
	}

	/* ---------------------------------------- */
}
