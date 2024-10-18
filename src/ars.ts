import { join } from '@std/path';

import type { Server, ServerConfig, ServerStatusUpdate } from './interfaces.ts';

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
	}

	/* ---------------------------------------- */

	start(): void {
		// read config to allow accessing values for command
		const decoder = new TextDecoder('utf-8');
		const fileContent = Deno.readFileSync(this.configPath);
		const config: ServerConfig = JSON.parse(decoder.decode(fileContent));

		const environment = Deno.env.get('ENVIRONMENT') || 'Production';
		let network, volumeSourceProfiles, volumeSourceServers, mountType;
		if (environment === 'Development') {
			network = 'default';
			volumeSourceProfiles = join(Deno.cwd(), 'profiles');
			volumeSourceServers = join(Deno.cwd(), 'servers');
			mountType = 'bind';
		} else if (environment === 'Production') {
			network = 'arsa_network';
			volumeSourceProfiles = 'arsa-profiles';
			volumeSourceServers = 'arsa-servers';
			mountType = 'volume';
		} else {
			network = 'arsa_network';
			volumeSourceProfiles = 'arsa-profiles';
			volumeSourceServers = 'arsa-servers';
			mountType = 'volume';
		}

		// starting Arma Reforger Server within a Docker Container
		// it's important to NOT combine multiple arguments like '-p' and '2001' in one argument '-p 2001'
		// otherwise it results in additional white spaces that break the call of docker with
		// '-p 2001: 2001' instead of '-p 2001:2001'
		const command = new Deno.Command('docker', {
			cwd: join(Deno.cwd(), 'ars'),
			args: [
				'run',
				'-d',
				'--rm',
				`--network=${network}`,
				'--hostname',
				`${this.uuid}`,
				'--mount',
				`type=${mountType},source=${volumeSourceProfiles},target=/ars/profiles`,
				'--mount',
				`type=${mountType},source=${volumeSourceServers},target=/ars/servers,readonly`,
				'-p',
				`${config.bindPort}:${config.bindPort}/udp`,
				'-p',
				`${config.a2s.port}:${config.a2s.port}/udp`,
				'-p',
				`${config.rcon.port}:${config.rcon.port}/udp`,
				'--name',
				`${this.uuid}`,
				'ars',
				'-config',
				`/ars/servers/${this.uuid}/config.json`,
				'-profile',
				`/ars/profiles/${this.uuid}`,
				'-maxFPS',
				'60',
			],
		});

		const { code, stdout, stderr } = command.outputSync();
		const output = (new TextDecoder().decode(stdout)).slice(0, -1);

		console.log(`old ars container id: ${this.arsContainerId}`);
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
			uuid: this.uuid,
			isRunning: this.isRunning,
		});

		console.log(`new ars container id: ${this.arsContainerId}`);

		console.log('Arma Reforger Server started.');

		this.checkInterval = setInterval(() => {
			if (this.isRunning) {
				if (!this.checkIsRunning()) {
					this.isRunning = false;
					this.setIsRunning(false);
					this.messageQueue.push({
						uuid: this.uuid,
						isRunning: this.isRunning,
					});
					clearInterval(this.checkInterval);
				}
			}
		}, 1_000);
	}

	/* ---------------------------------------- */

	stop(): void {
		const command = new Deno.Command('docker', {
			cwd: join(Deno.cwd(), 'ars'),
			args: [
				'kill',
				`${this.arsContainerId}`,
			],
		});

		const { code, stdout, stderr } = command.outputSync();
		const output = (new TextDecoder().decode(stdout)).slice(0, -1);

		console.log(`old ars container id: ${this.arsContainerId}`);
		// if code = 0 then reset container id
		console.log(`docker kill exit code: ${code}`);
		if (code === 0 && output.substring(0, 64) === this.arsContainerId) {
			this.isRunning = false;
			this.arsContainerId = '';
			console.log(`SUCCESS: ${output}`);
		} else {
			const error = (new TextDecoder().decode(stderr)).slice(0, -1);
			console.log(`ERROR: ${error}`);
		}

		this.setIsRunning(this.isRunning);

		this.messageQueue.push({
			uuid: this.uuid,
			isRunning: this.isRunning,
		});

		console.log(`new ars container id: ${this.arsContainerId}`);

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
				`${this.arsContainerId}`,
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
		const server: Server = JSON.parse(
			Deno.readTextFileSync(this.serverPath),
		);
		server.isRunning = isRunning;
		Deno.writeTextFileSync(
			this.serverPath,
			JSON.stringify(server, null, 2),
		);
	}
}
