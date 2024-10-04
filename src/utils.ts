import { join } from '@std/path';

export async function getServers() {
	const dir = join(Deno.cwd(), 'servers');
	const servers: object[] = [];
	for await (const dirEntry of Deno.readDir(dir)) {
		const fileContent = await Deno.readTextFile(
			join(dir, dirEntry.name, 'server.json'),
		);
		const server = JSON.parse(fileContent);
		servers.push(server);
		console.log(dirEntry.name);
	}

	return servers;
}

export async function getLogs(uuid: string) {
	const dir = join(Deno.cwd(), 'profiles', uuid, 'logs');
	const logs: string[] = [];
	for await (const dirEntry of Deno.readDir(dir)) {
		logs.push(dirEntry.name);
		console.log(dirEntry.name);
	}

	return logs;
}

export async function getLogFile(uuid: string, log: string, file: string) {
	const filePath = join(Deno.cwd(), 'profiles', uuid, 'logs', log, file);
	const fileContent = await Deno.readTextFile(filePath);

	return fileContent;
}

export async function getServer(uuid: string) {
	const dir = join(Deno.cwd(), 'servers');
	let server: object = Object;
	for await (const dirEntry of Deno.readDir(dir)) {
		if (dirEntry.name === uuid) {
			const fileContent = await Deno.readTextFile(
				join(dir, dirEntry.name, 'server.json'),
			);
			server = JSON.parse(fileContent);
			console.log(dirEntry.name);

			break;
		}
	}

	return server;
}
