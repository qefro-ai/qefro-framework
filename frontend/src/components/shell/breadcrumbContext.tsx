import { createContext, useContext, useMemo, useState, type ReactNode } from "react";

export type RecordCrumb = {
  id: string;
  label: string;
  parent?: { slug: string; id: string; label: string; entityLabel: string };
};

type Ctx = {
  record: RecordCrumb | null;
  setRecord: (crumb: RecordCrumb | null) => void;
};

const BreadcrumbRecordContext = createContext<Ctx>({
  record: null,
  setRecord: () => undefined,
});

export function BreadcrumbRecordProvider({ children }: { children: ReactNode }) {
  const [record, setRecord] = useState<RecordCrumb | null>(null);
  const value = useMemo(() => ({ record, setRecord }), [record]);
  return <BreadcrumbRecordContext.Provider value={value}>{children}</BreadcrumbRecordContext.Provider>;
}

export function useBreadcrumbRecord() {
  return useContext(BreadcrumbRecordContext);
}
