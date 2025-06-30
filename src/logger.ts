import AwsServerLogger from '@smooai/logger/AwsServerLogger';
import type Logger from '@smooai/logger/Logger';
import { isRunningInBrowser } from '@smooai/utils/env/index';

export const contextLogger = (): Logger => {
    if (isRunningInBrowser()) {
        const logger = new AwsServerLogger({
            name: 'smooai-fetch',
        });
        return logger;
    }

    const logger = new AwsServerLogger();
    if ('addLambdaContext' in logger) {
        logger.addLambdaContext();
    }
    return logger;
};
