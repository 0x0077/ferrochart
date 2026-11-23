import type { Locale } from '@/lib/i18n';
import { en, type Messages } from './en';
import { ja } from './ja';
import { ko } from './ko';
import { zh } from './zh';
import { zhTw } from './zh-tw';

export type { Messages };

const ALL: Record<Locale, Messages> = { en, zh, 'zh-tw': zhTw, ja, ko };

/** The interface text of `locale`. */
export const messages = (locale: Locale): Messages => ALL[locale];
