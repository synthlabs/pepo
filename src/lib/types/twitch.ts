import { HelixUser } from '@twurple/api';

export interface Channel {
	name: string;
	isLive?: boolean;
	streamInfo?: StreamInfo;
	user?: HelixUser;
}

export interface StreamInfo {
	title: string;
	viewers: number;
	startDate: Date;
}
