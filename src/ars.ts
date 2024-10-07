import { join } from '@std/path';

import type { ServerConfig } from './interfaces.ts';
import { defaultConfig } from './defaults.ts';

export class ArmaReforgerServer {
	uuid: string;
	arsContainerId: string;

	constructor(uuid: string) {
		if (!uuid) throw new Error('UUID is missing.');

		this.uuid = uuid;

		// read config to allow accessing values for command
		const decoder = new TextDecoder("utf-8");
		const fileContent = Deno.readFileSync(join(Deno.cwd(), 'servers', this.uuid, 'config.json'));
		const config: ServerConfig = JSON.parse(decoder.decode(fileContent));
		
		// starting Arma Reforger Server within a Docker Container
		// it's important to NOT combine multiple arguments like '-p' and '2001' in one argument '-p 2001'
		// otherwise it results in additional white spaces that break the call of docker with 
		// '-p 2001: 2001' instead of '-p 2001:2001'
		console.log(`Ports: ${config.bindPort}:${config.a2s.port}:${config.rcon.port}`);

		const command = new Deno.Command('docker', {
			cwd: join(Deno.cwd(), 'ars'),
			args: [
				'run', 
				'-d',
				'--rm',
				'--network=arsa_network', 
				'--hostname',
				`${this.uuid}`,
				'--mount',
				'type=volume,source=arsa-profiles,target=/ars/profiles',
				'--mount',
				'type=volume,source=arsa-servers,target=/ars/servers,readonly',
				'-p',
				`${config.bindPort}:${config.bindPort}`,
				'-p',
				`${config.a2s.port}:${config.a2s.port}`,
				'-p',
				`${config.rcon.port}:${config.rcon.port}`,
				'--name',
				`${this.uuid}`,
				'ars',
				'-config',
				`/ars/servers/${this.uuid}/config.json`,
				'-profile',
				`/ars/profiles/${this.uuid}`,
				'-maxFPS',
				'60'
			]
		});

		const { code, stdout, stderr } = command.outputSync();

		const output = new TextDecoder().decode(stdout);

		this.arsContainerId = '';

		console.log(`old ars container id: ${this.arsContainerId}`);
		// if code = 0 then the first 64 characters on stdout should be the container id
		console.log(`docker run exit code: ${code}`);
		if(code === 0) {
			this.arsContainerId = output.substring(0, 64);
			console.log(`SUCCESS: ${output}`);
		} else {
			const error = new TextDecoder().decode(stderr);
			console.log(`ERROR: ${error}`);
		}
		console.log(`new ars container id: ${this.arsContainerId}`);

		console.log('Arma Reforger Server started.');
	}

	stop(): void {
		const command = new Deno.Command('docker', {
			cwd: join(Deno.cwd(), 'ars'),
			args: [
				'kill',
				`${this.arsContainerId}`
			]
		});

		const { code, stdout, stderr } = command.outputSync();

		const output = new TextDecoder().decode(stdout);

		console.log(`old ars container id: ${this.arsContainerId}`);
		// if code = 0 then reset container id
		console.log(`docker kill exit code: ${code}`);
		if(code === 0 && output.substring(0, 64) === this.arsContainerId) {
			this.arsContainerId = '';
			console.log(`SUCCESS: ${output}`);
		} else {
			const error = new TextDecoder().decode(stderr);
			console.log(`ERROR: ${error}`);
		}
		console.log(`new ars container id: ${this.arsContainerId}`);

		console.log('Arma Refoger Server stopped.');
	}

	isRunning(): boolean {
		const command = new Deno.Command('docker', {
			cwd: join(Deno.cwd(), 'ars'),
			args: [
				'container',
				'inspect',
				'-f',
				'{{.State.Running}}',
				`${this.arsContainerId}`
			]
		});

		const { code, stdout, stderr } = command.outputSync();

		const output = new TextDecoder().decode(stdout);

		if(code === 0 && output.substring(0, 4) === 'true') {
			//console.log(`SUCCESS: ${output}`);
			return true;
		} else {
			const error = new TextDecoder().decode(stderr);
			console.log(`ERROR: ${error}`);
			return false;
		}
	}
}
