export enum LoginStatus {
  Ms = 'Ms',
  Xbox = 'Xbox',
  XboxSecurity = 'XboxSecurity',
  Mc = 'Mc'
}

export const ACCOUNT_LOGIN_STATUS_EVENT = 'account-login-status';

export type VersionCheckStatus =
  | 'VersionManifestCheck'
  | 'VersionManifestDownload'
  | 'AssetsManifestCheck'
  | 'AssetsManifestDownload'
  | 'JavaManifestCheck'
  | 'JavaManifestDownload'
  | 'ClientCheck'
  | 'ClientDownload'
  | {
      AssetsCheck: [number, number];
    }
  | {
      AssetsDownload: [number, number];
    }
  | {
      JavaCheck: [number, number];
    }
  | {
      JavaDownload: [number, number];
    }
  | {
      NativeLibraryCheck: [number, number];
    }
  | {
      NativeLibraryDownload: [number, number];
    }
  | {
      LibraryCheck: [number, number];
    }
  | {
      LibraryDownload: [number, number];
    }
  | 'ModLoaderMeta'
  | { ModLoaderFilesCheck: [number, number] }
  | { ModLoaderFilesDownload: [number, number] }
  | 'ModLoaderPreprocess'
  | 'ModLoaderPreprocessDone'
  | 'Done';

export type VersionCheckData = {
  data: VersionCheckStatus;
  id: number;
};

export const VERSION_CHECK_STATUS_EVENT = 'version-check-status';
export const INSTANCE_CRASH_EVENT = 'instance-crash';
export const MANIFEST_REFRESH_ERROR_EVENT = 'manifest-refresh-error';

//10 minutes
export const TOAST_DURATION = 600000;
