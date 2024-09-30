import { join } from '@std/path';

export async function getServers(dir: string) {
    const servers: object[] = [];
    for await (const dirEntry of Deno.readDir(dir)) {
        const fileContent = await Deno.readTextFile(join(dir, dirEntry.name));
        const config = JSON.parse(fileContent);
        servers.push(config);
        console.log(dirEntry.name);
    };

    return servers;
}