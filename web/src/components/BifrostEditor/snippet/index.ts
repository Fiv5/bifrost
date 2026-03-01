import operator from './operator';
import dynamicProvider, {
  updateDynamicData,
  getDynamicData,
  type DynamicCompletionData,
} from './dynamic';

export { updateDynamicData, getDynamicData, dynamicProvider };
export type { DynamicCompletionData };

export default {
  operator,
  dynamicProvider,
};
