import { join } from '@std/path';

export async function getServers(dir: string) {
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

export async function getServer(dir: string, uuid: string) {
	let server: object = Object;
	for await (const dirEntry of Deno.readDir(dir)) {
		if(dirEntry.name === uuid) {
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
