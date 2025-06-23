import AwsServerLogger from '@smooai/logger/AwsServerLogger';
import BrowserLogger from '@smooai/logger/BrowserLogger';
import type Logger from '@smooai/logger/Logger';
import { isRunningInBrowser } from '@smooai/utils/env/index';

export const contextLogger = (): Logger => {
    if (isRunningInBrowser()) {
        const logger = new BrowserLogger({
            name: 'smooai-fetch',
        });
        return logger;
    }

    const logger = new AwsServerLogger();
    logger.addLambdaContext();
    return logger;
};
