import { ErrorController, logHandledError } from '$utils/inbound';

// One controller for native, window, and chat send failures; error-host.svelte presents it.
export const appErrors = new ErrorController({
	log: (error) => logHandledError(error.message, error.target ?? 'app')
});
