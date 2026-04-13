import { useCallback } from 'react';
import { message } from 'antd';
import {
  exportRules,
  exportNetwork,
  exportScripts,
  exportValues,
  exportTemplates,
  downloadFile,
  formatExportFilename,
} from '../api/bifrost-file';
import type {
  BifrostFileType,
  ExportRulesRequest,
  ExportNetworkRequest,
  ExportScriptRequest,
  ExportValuesRequest,
  ExportTemplateRequest,
} from '../api/bifrost-file';

type ExportRequest =
  | ExportRulesRequest
  | ExportNetworkRequest
  | ExportScriptRequest
  | ExportValuesRequest
  | ExportTemplateRequest;

export function useExportBifrost() {
  const exportFile = useCallback(
    async (
      fileType: BifrostFileType,
      request: ExportRequest,
      customFilename?: string
    ) => {
      try {
        let content: string;
        let count: number | undefined;

        switch (fileType) {
          case 'rules':
            content = await exportRules(request as ExportRulesRequest);
            count = (request as ExportRulesRequest).rule_names.length;
            break;
          case 'network':
            content = await exportNetwork(request as ExportNetworkRequest);
            count = (request as ExportNetworkRequest).record_ids.length;
            break;
          case 'script':
            content = await exportScripts(request as ExportScriptRequest);
            count = (request as ExportScriptRequest).script_names.length;
            break;
          case 'values': {
            const valuesReq = request as ExportValuesRequest;
            content = await exportValues(valuesReq);
            count = valuesReq.value_names?.length;
            break;
          }
          case 'template': {
            const templateReq = request as ExportTemplateRequest;
            content = await exportTemplates(templateReq);
            count =
              templateReq.request_ids?.length || templateReq.group_ids?.length;
            break;
          }
          default:
            throw new Error(`Unknown file type: ${fileType}`);
        }

        const filename = customFilename || formatExportFilename(fileType, count);
        downloadFile(content, filename);
        message.success(`Exported as ${filename}`);
      } catch (error) {
        message.error(`Export failed: ${error}`);
      }
    },
    []
  );

  return { exportFile };
}

export default useExportBifrost;
