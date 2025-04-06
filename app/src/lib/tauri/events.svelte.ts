export enum LoginStatus {
  Ms = 'Ms',
  Xbox = 'Xbox',
  XboxSecurity = 'XboxSecurity',
  Mc = 'Mc'
}

export const ACCOUNT_LOGIN_STATUS_EVENT = 'account-login-status';

export type VersionCheckStatus =
  | {
      Manifest: number;
    }
  | {
      Assets: [number, number];
    }
  | {
      Java: [number, number];
    }
  | {
      NativeLibrary: [number, number];
    }
  | {
      Library: [number, number];
    }
  | {
      Java: [number, number];
    }
  | 'Client'
  | 'Done';

export const VERSION_CHECK_STATUS_EVENT = 'version-check-status';

//10 minutes
export const TOAST_DURATION = 600000;
