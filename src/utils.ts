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
