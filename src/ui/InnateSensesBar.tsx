import {useEffect, useState} from 'react';
import {t} from '../domain/packs';

export type DayPart = 'morning' | 'day' | 'evening' | 'night';

export interface InnateSensesSnapshot {
  localTime: string;
  timeZone: string;
  dayPart: DayPart;
  online: boolean;
  batteryPercent?: number;
  charging?: boolean;
}

interface BatteryManagerLike extends EventTarget {
  level: number;
  charging: boolean;
}

function computeDayPart(hour: number): DayPart {
  if (hour >= 5 && hour < 12) return 'morning';
  if (hour >= 12 && hour < 18) return 'day';
  if (hour >= 18 && hour < 22) return 'evening';
  return 'night';
}

export function readInnateSenses(now = new Date()): InnateSensesSnapshot {
  const hours = String(now.getHours()).padStart(2, '0');
  const minutes = String(now.getMinutes()).padStart(2, '0');
  let timeZone = 'Local';
  try {
    timeZone = Intl.DateTimeFormat().resolvedOptions().timeZone || 'Local';
  } catch {
    timeZone = 'Local';
  }
  const online = typeof navigator !== 'undefined' && typeof navigator.onLine === 'boolean' ? navigator.onLine : true;
  return {
    localTime: `${hours}:${minutes}`,
    timeZone,
    dayPart: computeDayPart(now.getHours()),
    online,
  };
}

export function InnateSensesBar({locale, compact = false}: {locale: string; compact?: boolean}) {
  const text = (key: string) => t(locale, key);
  const [senses, setSenses] = useState<InnateSensesSnapshot>(() => readInnateSenses());

  useEffect(() => {
    let active = true;
    let battery: BatteryManagerLike | undefined;

    const refresh = () => {
      if (!active) return;
      const base = readInnateSenses();
      setSenses(prev => ({
        ...base,
        batteryPercent: battery ? Math.round(battery.level * 100) : prev.batteryPercent,
        charging: battery ? battery.charging : prev.charging,
      }));
    };

    window.addEventListener('online', refresh);
    window.addEventListener('offline', refresh);
    window.addEventListener('focus', refresh);
    document.addEventListener('visibilitychange', refresh);

    const nav = navigator as Navigator & {getBattery?: () => Promise<BatteryManagerLike>};
    if (typeof nav.getBattery === 'function') {
      void nav.getBattery().then(mgr => {
        if (!active) return;
        battery = mgr;
        refresh();
        mgr.addEventListener('levelchange', refresh);
        mgr.addEventListener('chargingchange', refresh);
      }).catch(() => {
        // Battery API unavailable on this host; stay honest without fabricating values.
      });
    }

    return () => {
      active = false;
      window.removeEventListener('online', refresh);
      window.removeEventListener('offline', refresh);
      window.removeEventListener('focus', refresh);
      document.removeEventListener('visibilitychange', refresh);
      if (battery) {
        battery.removeEventListener('levelchange', refresh);
        battery.removeEventListener('chargingchange', refresh);
      }
    };
  }, []);

  const dayPartLabel = text('senses.dayPart.' + senses.dayPart);
  const netLabel = text(senses.online ? 'senses.online' : 'senses.offline');

  return <div className={'senses-bar' + (compact ? ' compact' : '')} aria-label={text('senses.title')}>
    <span className="sense-chip" title={senses.timeZone}>
      {dayPartLabel} · {senses.localTime}
    </span>
    <span className={'sense-chip ' + (senses.online ? 'sense-ok' : 'sense-warn')}>
      {netLabel}
    </span>
    {typeof senses.batteryPercent === 'number' && <span className="sense-chip">
      {text('senses.power')}: {senses.batteryPercent}%{senses.charging ? ' ⚡' : ''}
    </span>}
  </div>;
}
