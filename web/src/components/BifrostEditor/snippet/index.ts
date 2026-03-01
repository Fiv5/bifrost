import operator from './operator';
import dynamicProvider, {
  updateDynamicData,
  getDynamicData,
  type DynamicCompletionData,
  type ReferenceLocation,
  type ReferenceType,
  type NavigationType,
} from './dynamic';
import {
  hoverProvider,
  definitionProvider,
  linkProvider,
  setNavigateCallback,
  type NavigateCallback,
} from './hover';

export { updateDynamicData, getDynamicData, dynamicProvider };
export { hoverProvider, definitionProvider, linkProvider, setNavigateCallback };
export type { DynamicCompletionData, ReferenceLocation, ReferenceType, NavigationType, NavigateCallback };

export default {
  operator,
  dynamicProvider,
  hoverProvider,
  definitionProvider,
  linkProvider,
};
