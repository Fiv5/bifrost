import { App } from "antd";

export function useAppModal() {
  const { modal } = App.useApp();
  return modal;
}
