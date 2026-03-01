import { useEffect, useRef } from 'react';
import { useValuesStore } from '../stores/useValuesStore';
import { useScriptsStore } from '../stores/useScriptsStore';
import { updateDynamicData } from '../components/BifrostEditor';

let initialized = false;

export function useEditorCompletion() {
  const initRef = useRef(false);

  useEffect(() => {
    if (initRef.current || initialized) {
      return;
    }
    initRef.current = true;
    initialized = true;

    useScriptsStore.getState().fetchScripts();

    const unsubValues = useValuesStore.subscribe((state) => {
      updateDynamicData({
        values: state.values.map((v) => v.name),
      });
    });

    const unsubScripts = useScriptsStore.subscribe((state) => {
      updateDynamicData({
        requestScripts: state.requestScripts.map((s) => s.name),
        responseScripts: state.responseScripts.map((s) => s.name),
      });
    });

    const valuesState = useValuesStore.getState();
    if (valuesState.values.length > 0) {
      updateDynamicData({
        values: valuesState.values.map((v) => v.name),
      });
    }

    return () => {
      unsubValues();
      unsubScripts();
      initialized = false;
    };
  }, []);
}

export function resetEditorCompletion() {
  initialized = false;
  updateDynamicData({
    values: [],
    requestScripts: [],
    responseScripts: [],
  });
}
