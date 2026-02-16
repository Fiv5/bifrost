import { useMemo } from 'react';
import type { KeyValueItem } from '../../../types';

export const useTableCalculateIdList = (
  list: KeyValueItem[],
  prefix = ''
): KeyValueItem[] => {
  const calcIdList = useMemo(() => {
    const nextList = JSON.parse(JSON.stringify(list)) as KeyValueItem[];
    let recordId = 0;

    const loop = (recordList: KeyValueItem[]) => {
      recordList.forEach((item) => {
        item.id = prefix + String(recordId++);
        if (item.children) {
          loop(item.children);
        }
      });
    };
    loop(nextList);
    return nextList;
  }, [list, prefix]);

  return calcIdList;
};
