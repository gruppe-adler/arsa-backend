import { join } from '@std/path';
import { Server } from './interfaces.ts';

export async function getServers() {
	const dir = join(Deno.cwd(), 'servers');
	const servers: Server[] = [];
	for await (const dirEntry of Deno.readDir(dir)) {
		const serverFileContent = await Deno.readTextFile(
			join(dir, dirEntry.name, 'server.json'),
		);
		const server: Server = JSON.parse(serverFileContent);
		const configFileContent = await Deno.readTextFile(
			join(dir, dirEntry.name, 'config.json'),
		);
		const config = JSON.parse(configFileContent);
		server.config = config;
		servers.push(server);
		console.log(dirEntry.name);
	}

	return servers;
}

export async function getServer(uuid: string) {
	const dir = join(Deno.cwd(), 'servers');
	let server: Server = {} as Server;
	for await (const dirEntry of Deno.readDir(dir)) {
		if (dirEntry.name === uuid) {
			const serverFileContent = await Deno.readTextFile(
				join(dir, dirEntry.name, 'server.json'),
			);
			server = JSON.parse(serverFileContent);

			const configFileContent = await Deno.readTextFile(
				join(dir, dirEntry.name, 'config.json'),
			);
			const config = JSON.parse(configFileContent);
			server.config = config;

			console.log(dirEntry.name);

			break;
		}
	}

	return server;
}

export async function getLogs(uuid: string) {
	const dir = join(Deno.cwd(), 'profiles', uuid, 'logs');
	const logs: string[] = [];
	try {
		for await (const dirEntry of Deno.readDir(dir)) {
			logs.push(dirEntry.name);
			console.log(dirEntry.name);
		}
	} catch (error) {
		console.log('No log entries available');
	}

	return logs;
}

export async function getLogFile(uuid: string, log: string, file: string) {
	const filePath = join(Deno.cwd(), 'profiles', uuid, 'logs', log, file);
	const fileContent = await Deno.readTextFile(filePath);

	return fileContent;
}
