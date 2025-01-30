import { join } from '@std/path';
import { directoryExists, fileExists } from './utils.ts';
import { Log, ResultLogs } from './interfaces.ts';

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
	const logsDir = join(Deno.cwd(), 'profiles', uuid, 'logs');

	const resultLogs: ResultLogs = {
		success: false,
		logs: [],
		containsCrashReportsLog: false,
	};

	if (!await directoryExists(logsDir)) return resultLogs;

	try {
		for await (const dirEntry of Deno.readDir(logsDir)) {
			console.log(dirEntry.name);
			const log: Log = {
				dir: dirEntry.name,
				containsConsoleLog: false,
				containsScriptLog: false,
				containsErrorLog: false,
				containsCrashLog: false,
			};

			if (!await directoryExists(join(logsDir, dirEntry.name))) continue;

			if (await fileExists(join(logsDir, dirEntry.name, 'console.log'))) {
				log.containsConsoleLog = true;
			}
			if (await fileExists(join(logsDir, dirEntry.name, 'script.log'))) {
				log.containsScriptLog = true;
			}
			if (await fileExists(join(logsDir, dirEntry.name, 'error.log'))) {
				log.containsErrorLog = true;
			}
			if (await fileExists(join(logsDir, dirEntry.name, 'crash.log'))) {
				log.containsCrashLog = true;
			}

			resultLogs.logs.push(log);
		}
	} catch (error) {
		console.log(error);
	}

	if (await fileExists(join(logsDir, 'CrashReports.log'))) {
		resultLogs.containsCrashReportsLog = true;
	}

	resultLogs.success = true;

	return resultLogs;
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

export async function getCrashReportsLog(uuid: string) {
	const filePath = join(
		Deno.cwd(),
		'profiles',
		uuid,
		'logs',
		'CrashReports.log',
	);
	if (!await fileExists(filePath)) return '';

	let fileContent = '';

	try {
		fileContent = await Deno.readTextFile(filePath);
	} catch (error) {
		console.log(error);
	}

	return fileContent;
}
