import { join } from '@std/path';
import { Server } from './interfaces.ts';
import { directoryExists } from './utils.ts';

export async function getServers() {
	const dir = join(Deno.cwd(), 'servers');

	const servers: Server[] = [];

	if (!await directoryExists(dir)) return servers;

	for await (const dirEntry of Deno.readDir(dir)) {
		try {
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
		} catch (error) {
			console.log(error);
		}
	}

	return servers;
}

export async function getServer(uuid: string): Promise<Server | null> {
	const serverPath = join(Deno.cwd(), 'servers', uuid);

	let server: Server = {} as Server;

    if (! directoryExists(serverPath)) { return server }

    try {
        const serverFileContent = await Deno.readTextFile(
            join(serverPath, 'server.json'),
        );
        server = JSON.parse(serverFileContent);
    
        const configFileContent = await Deno.readTextFile(
            join(serverPath, 'config.json'),
        );
        const config = JSON.parse(configFileContent);
        server.config = config;
    } catch (error) {
        console.log(error);
        return null;
    }

	return server;
}
