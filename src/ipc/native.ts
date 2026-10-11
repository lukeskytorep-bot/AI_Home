import {invoke} from '@tauri-apps/api/core';import {listen} from '@tauri-apps/api/event';import type {Transport,AppEvent} from '../domain/types';
export const nativeTransport:Transport={mode:'desktop',dispatch:r=>invoke('home_dispatch',{request:r}),subscribe:handler=>listen<AppEvent>('home:app-event',event=>handler(event.payload))};
