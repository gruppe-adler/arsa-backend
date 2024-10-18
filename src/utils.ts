import { join } from '@std/path';

export async function createSharedFolders() {
	const serversDir = join(Deno.cwd(), 'servers');
	const profilesDir = join(Deno.cwd(), 'profiles');
	try {
		await Deno.mkdir(profilesDir, { recursive: true });
	} catch (error) {
		if (error instanceof Deno.errors.AlreadyExists) {
			console.log(`Directory ${profilesDir} already exists.`);
		} else {
			throw error;
		}
	}
	try {
		await Deno.mkdir(serversDir, { recursive: true });
	} catch (error) {
		if (error instanceof Deno.errors.AlreadyExists) {
			console.log(`Directory ${serversDir} already exists.`);
		} else {
			throw error;
		}
	}
}

export async function fileExists(filename: string): Promise<boolean> {
	try {
		const fileInfo = await Deno.stat(filename);
		// successful, file or directory must exist
		return fileInfo.isFile;
	} catch (error) {
		if (error instanceof Deno.errors.NotFound) {
			// file or directory does not exist
			return false;
		} else {
			// unexpected error, maybe permissions, pass it along
			throw error;
		}
	}
}

export async function directoryExists(filename: string): Promise<boolean> {
	try {
		const fileInfo = await Deno.stat(filename);
		// successful, file or directory must exist
		return fileInfo.isDirectory;
	} catch (error) {
		if (error instanceof Deno.errors.NotFound) {
			// file or directory does not exist
			return false;
		} else {
			// unexpected error, maybe permissions, pass it along
			throw error;
		}
	}
}
