import { join } from '@std/path';
import { directoryExists, fileExists } from './utils.ts';

export function isValidLogDirName(logDirName: string): boolean {
	const logDirNameRegEx = new RegExp(
		'^logs_([0-9]{4})-([0-9]{2})-([0-9]{2})_([0-9]{2})-([0-9]{2})-([0-9]{2})$',
	);
	const logDirNameResult = logDirNameRegEx.exec(logDirName);

	if (logDirNameResult === null) return false;

	const year = logDirNameResult[1];
	const month = logDirNameResult[2];
	const day = logDirNameResult[3];
	const hours = logDirNameResult[4];
	const minutes = logDirNameResult[5];
	const seconds = logDirNameResult[6];

	const isoDateString =
		`${year}-${month}-${day}T${hours}:${minutes}:${seconds}.000Z`;

	const date = Date.parse(isoDateString);

	return (isNaN(date)) ? false : true;
}

export async function getLogs(uuid: string) {
	const dir = join(Deno.cwd(), 'profiles', uuid, 'logs');

	const logs: string[] = [];

	if (!await directoryExists(dir)) return logs;

	try {
		for await (const dirEntry of Deno.readDir(dir)) {
			logs.push(dirEntry.name);
		}
	} catch (error) {
		console.log(error);
	}

	return logs;
}

export async function getLogFile(uuid: string, log: string, file: string) {
	const filePath = join(Deno.cwd(), 'profiles', uuid, 'logs', log, file);
	if (!await fileExists(filePath)) return '';

	let fileContent = '';

	try {
		fileContent = await Deno.readTextFile(filePath);
	} catch (error) {
		console.log(error);
	}

	return fileContent;
}
