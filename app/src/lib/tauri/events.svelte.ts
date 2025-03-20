export enum LoginStatus {
  Ms = 'Ms',
  Xbox = 'Xbox',
  XboxSecurity = 'XboxSecurity',
  Mc = 'Mc'
}

export const ACCOUNT_LOGIN_STATUS_EVENT = 'account-login-status';

export enum VersionCheckStatus {
  Manifest = 'Manifest',
  Assets = 'Assets',
  Java = 'Java'
}

export const VERSION_CHECK_STATUS_EVENT = 'version-check-status';
