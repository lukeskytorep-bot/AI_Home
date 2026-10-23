import {it,expect} from 'vitest';
import {validDto} from '../src/domain/schema';
import {t} from '../src/domain/packs';
it('permanent deletion tombstone uses an allowed terminal Error shape',()=>{
 expect(validDto('Error',{code:'POLICY_DENIED',messageKey:'error.permanently_deleted',retryable:false})).toBe(true);
});
it('permanent deletion reason is localized in EN and PL',()=>{
 for(const locale of ['en','pl']) expect(t(locale,'error.permanently_deleted')).not.toBe('error.permanently_deleted');
});
