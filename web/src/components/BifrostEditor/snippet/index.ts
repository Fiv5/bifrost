import operator from './operator';
import {
  dynamicProvider,
  updateDynamicData,
  getDynamicData,
  setLocalVariablesGetter,
  type DynamicCompletionData,
  type ReferenceLocation,
  type ReferenceType,
  type NavigationType,
  type LocalVariableGetter,
} from './dynamic';
import {
  hoverProvider,
  definitionProvider,
  setNavigateCallback,
  getNavigateCallback,
  setLocalVariables,
  executePendingNavigation,
  type NavigateCallback,
  type LocalVariableDefinition,
} from './hover';
import {
  validateRules,
  setValidationMarkers,
  clearValidationMarkers,
  createDebouncedValidator,
  type ParseError,
  type ValidationResult,
  type DebouncedValidator,
  type GlobalValuesGetter,
} from './validation';

export { updateDynamicData, getDynamicData, dynamicProvider, setLocalVariablesGetter };
export { hoverProvider, definitionProvider, setNavigateCallback, getNavigateCallback, setLocalVariables, executePendingNavigation };
export { validateRules, setValidationMarkers, clearValidationMarkers, createDebouncedValidator };
export type { DynamicCompletionData, ReferenceLocation, ReferenceType, NavigationType, NavigateCallback, LocalVariableDefinition, LocalVariableGetter };
export type { ParseError, ValidationResult, DebouncedValidator, GlobalValuesGetter };

export default {
  operator,
  dynamicProvider,
  hoverProvider,
  definitionProvider,
};
