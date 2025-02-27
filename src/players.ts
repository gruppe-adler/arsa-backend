import { join } from '@std/path';
import type { PlayerIdentityId } from './interfaces.ts';
import { fileExists } from './utils.ts';

export async function getPlayersFromLog(
	logFilePath: string,
): Promise<PlayerIdentityId[]> {
	const playerIdentityIds: PlayerIdentityId[] = [];

	if (! fileExists(logFilePath)) { return playerIdentityIds }

	try {
		const log = await Deno.readTextFile(logFilePath);
		const logArray = log.split('\n');

		logArray.forEach((line) => {
			const itentityIdRegEx = new RegExp(
				'IdentityId=([0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12})',
			);
			const nameRegEx = new RegExp('Name=(.*),');
			const identityIdResult = itentityIdRegEx.exec(line);
			const nameResult = nameRegEx.exec(line);

			if ((identityIdResult !== null) && (nameResult !== null)) {
				playerIdentityIds.push({
					name: nameResult[1],
					identityId: identityIdResult[1],
				});
			}
		});
	} catch (error) {
		console.log(error);
	}

	return playerIdentityIds;
}

export async function addToKnownPlayers(
	uuid: string,
	newPlayers: PlayerIdentityId[],
): Promise<void> {
	const knownPlayersFilePath = join(
		Deno.cwd(),
		'servers',
		uuid,
		'known-players.json',
	);

	let players: PlayerIdentityId[] = [];

	try {
		const fileContent = await Deno.readTextFile(knownPlayersFilePath);
		players = JSON.parse(fileContent);
	} catch (error) {
		if (! (error instanceof Deno.errors.NotFound)) { console.log(error) }
	}

	newPlayers.forEach((newPlayer) => {
		if (
			players.findIndex((player) =>
				player.identityId === newPlayer.identityId
			) === -1
		) {
			players.push(newPlayer);
		}
	});

	await Deno.writeTextFile(
		knownPlayersFilePath,
		JSON.stringify(players, null, 2),
	);
}

export async function getKnownPlayers(
	uuid: string,
): Promise<PlayerIdentityId[]> {
	const knownPlayersFilePath = join(
		Deno.cwd(),
		'servers',
		uuid,
		'known-players.json',
	);

	let knownPlayers: PlayerIdentityId[] = [];

	if(! fileExists(knownPlayersFilePath)) { return knownPlayers }

	try {
		const fileContent = await Deno.readTextFile(knownPlayersFilePath);
		knownPlayers = JSON.parse(fileContent);
	} catch (error) {
		console.log(error);
	}

	return knownPlayers;
}
