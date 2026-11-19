export interface UnitFirmware {
  id: string;
  version: string;
}

const activeUnitKey = "fm-mate.activeUnitId";

export function setActiveUnit(unitId: string | null) {
  if (unitId) sessionStorage.setItem(activeUnitKey, unitId);
  else sessionStorage.removeItem(activeUnitKey);
}

export function getUnitFirmware(unitId: string): UnitFirmware | null {
  const value = localStorage.getItem(`fm-mate.unitFirmware.${unitId}`);
  if (!value) return null;
  const parsed: unknown = JSON.parse(value);
  if (
    typeof parsed !== "object" || parsed === null ||
    !("id" in parsed) || typeof parsed.id !== "string" || !parsed.id ||
    !("version" in parsed) || typeof parsed.version !== "string" || !parsed.version
  ) throw new Error(`Invalid saved firmware association for unit ${unitId}.`);
  return { id: parsed.id, version: parsed.version };
}

export function setUnitFirmware(unitId: string, firmware: UnitFirmware) {
  localStorage.setItem(`fm-mate.unitFirmware.${unitId}`, JSON.stringify(firmware));
}

export function clearUnitFirmware(unitId: string) {
  localStorage.removeItem(`fm-mate.unitFirmware.${unitId}`);
}

export function getUnitBackupTarget(unitId: string): string | null {
  return localStorage.getItem(`fm-mate.unitBackupTarget.${unitId}`);
}

export function setUnitBackupTarget(unitId: string, target: string) {
  const key = `fm-mate.unitBackupTarget.${unitId}`;
  if (target) localStorage.setItem(key, target);
  else localStorage.removeItem(key);
}

export function getActiveUnitFirmware(): UnitFirmware | null {
  const unitId = sessionStorage.getItem(activeUnitKey);
  return unitId ? getUnitFirmware(unitId) : null;
}
