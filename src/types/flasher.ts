import type { InstalledSave } from "./saves";

export type MFlasherRequestPayload =
  | {
      type: "flash-firmware";
      firmwareId: string;
      firmwareName: string;
      version: string;
    }
  | {
      type: "switch-firmware";
      firmwareId: string;
      firmwareName: string;
      version: string;
      automaticallyBackup: boolean;
    }
  | { type: "flash-save"; save: InstalledSave }
  | { type: "backup" }
  | { type: "flash-bootloader" };

export type MFlasherRequest = MFlasherRequestPayload & { requestId: number };

export interface MFlasherLcdState {
  title: string;
  message: string;
  connection: string;
  hint: string;
  progress: number | null;
  messageClass: string;
}
