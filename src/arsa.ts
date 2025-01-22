import { join } from '@std/path';
import { publicIpv4 } from 'npm:public-ip@7.0.1';
import {
	ArsStatus,
	ArsStatusUpdate,
	DockerInspect,
	Server,
	ServerMessage,
	ServerStatusUpdate,
} from './interfaces.ts';
import { ArmaReforgerServer } from './ars.ts';

export class ArmaReforgerServerAdmin {
	arsStatus: ArsStatus = ArsStatus.UNKNOWN;
	arsStatusTimestamp: number = Date.now();
	arsInspect: DockerInspect | undefined;
	arsList: ArmaReforgerServer[] = [];
	publicIp: string = '';
	messageQueue: ServerStatusUpdate[] = [];

	constructor() {
		this.inspectARS();

		// deno-lint-ignore no-this-alias
		const self = this;
		setInterval(() => {
			self.inspectARS();
			self.sendArsStatusUpdate();
		}, 30_000);

		this.getArsInstances();

		this.getPublicIp();
	}

	setArsStatus(status: ArsStatus) {
		this.arsStatus = status;
		this.arsStatusTimestamp = Date.now();
	}

	async getArsInstances() {
		// INIT: read all existing server.json and create ars instances
		const dir = join(Deno.cwd(), 'servers');

		try {
			for await (const dirEntry of Deno.readDir(dir)) {
				const fileContent = await Deno.readTextFile(
					join(dir, dirEntry.name, 'server.json'),
				);
				const server = JSON.parse(fileContent) as Server;

				const ars = new ArmaReforgerServer(server.uuid);
				this.arsList.push(ars);

				console.log(
					`INIT: Adding Arma Reforger Server with UUID: ${server.uuid}`,
				);
			}
		} catch (error) {
			console.log(error);
		}
	}

	async getPublicIp() {
		this.publicIp = await publicIpv4();
	}

	inspectARS() {
		const commandDelete = new Deno.Command('docker', {
			cwd: join(Deno.cwd(), 'ars'),
			args: ['image', 'inspect', '-f', 'json', 'ars'],
		});
		const commandInspectOutput = commandDelete.outputSync();

		if (commandInspectOutput.success) {
			const jsonString = new TextDecoder().decode(
				commandInspectOutput.stdout,
			);
			// console.log(jsonString);
			console.log('ARS inspected successfully.');
			this.arsInspect = JSON.parse(jsonString)[0];

			this.setArsStatus(ArsStatus.AVAILABLE);
		} else {
			this.arsInspect = undefined;
			this.setArsStatus(ArsStatus.UNAVAILABLE);
		}
		this.sendArsStatusUpdate();
	}

	async recreateARS() {
		this.setArsStatus(ArsStatus.RECREATING);
		this.sendArsStatusUpdate();

		// STEP 1
		this.logAndSendMessage('Stopping all ars instances...');
		this.arsList.forEach((ars) => {
			if (ars.isRunning) {
				ars.stop();
				this.logAndSendMessage(`Server with uuid ${ars.uuid} stopped.`);
			}
		});
		this.logAndSendMessage('All ars instances stopped.');

		// STEP 2
		this.logAndSendMessage('Deleting current ars image...');
		const commandDelete = new Deno.Command('docker', {
			cwd: join(Deno.cwd(), 'ars'),
			args: ['image', 'rm', 'ars'],
		});
		const commandDeleteOutput = await commandDelete.output();
		if (commandDeleteOutput.success) {
			this.logAndSendMessage('Current ars image deleted.');
		} else {
			this.logAndSendMessage(
				`Deleting current ars image failed with return code ${commandDeleteOutput.code}`,
			);
			const stdErrString = new TextDecoder().decode(
				commandDeleteOutput.stderr,
			);
			this.logAndSendMessage(stdErrString);
			// this is not necessarily a failure, therefore we do not break here
			//this.setArsStatus(ArsStatus.RECREATING_FAILURE);
			//this.sendArsStatusUpdate();
			//return;
		}

		// STEP 3
		this.logAndSendMessage('Pulling latest steamcmd image...');
		const commandPull = new Deno.Command('docker', {
			cwd: join(Deno.cwd(), 'ars'),
			args: ['pull', 'steamcmd/steamcmd:latest'],
		});
		const commandPullOutput = await commandPull.output();
		if (commandPullOutput.success) {
			this.logAndSendMessage('Latest steamcmd image pulled.');
		} else {
			this.logAndSendMessage(
				`Pulling latest steamcmd image failed with return code ${commandPullOutput.code}`,
			);
			const stdErrString = new TextDecoder().decode(
				commandPullOutput.stderr,
			);
			this.logAndSendMessage(stdErrString);
			this.setArsStatus(ArsStatus.RECREATING_FAILURE);
			this.sendArsStatusUpdate();
			return;
		}

		// STEP 4
		this.logAndSendMessage('Building new ars image...');
		const commandBuild = new Deno.Command('docker', {
			cwd: join(Deno.cwd(), 'ars'),
			args: [
				'buildx',
				'build',
				'--no-cache',
				'-t',
				'ars',
				'https://github.com/gruppe-adler/arsa-backend.git#main:ars',
			],
		});
		const commandBuildOutput = await commandBuild.output();
		if (commandBuildOutput.success) {
			this.logAndSendMessage('New ars image built.');
		} else {
			this.logAndSendMessage(
				`Building new ars image failed with return code ${commandBuildOutput.code}`,
			);
			const stdErrString = new TextDecoder().decode(
				commandBuildOutput.stderr,
			);
			this.logAndSendMessage(stdErrString);
			this.setArsStatus(ArsStatus.RECREATING_FAILURE);
			this.sendArsStatusUpdate();
			return;
		}

		// STEP 5
		this.logAndSendMessage('Inspecting ars image...');
		const commandInspect = new Deno.Command('docker', {
			cwd: join(Deno.cwd(), 'ars'),
			args: ['image', 'inspect', '-f', 'json', 'ars'],
		});
		const commandInspectOutput = await commandInspect.output();
		if (commandInspectOutput.success) {
			this.logAndSendMessage('Successfully inspecting ars image.');
		} else {
			this.arsInspect = undefined;
			this.logAndSendMessage(
				`Inspecting ars image failed with return code ${commandInspectOutput.code}`,
			);
			const stdErrString = new TextDecoder().decode(
				commandInspectOutput.stderr,
			);
			this.logAndSendMessage(stdErrString);
			this.setArsStatus(ArsStatus.RECREATING_FAILURE);
			this.sendArsStatusUpdate();
			return;
		}

		this.setArsStatus(ArsStatus.AVAILABLE);
		this.sendArsStatusUpdate();
	}

	logAndSendMessage(message: string) {
		console.log(message);
		this.messageQueue.push({
			type: 'message',
			message: message,
		} as ServerMessage);
	}

	sendArsStatusUpdate() {
		this.messageQueue.push({
			type: 'arsStatusUpdate',
			arsStatus: this.arsStatus,
		} as ArsStatusUpdate);
	}
}
