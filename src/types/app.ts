export type AppEntry = {
  id: string;
  name: string;
  description?: string;
  icon?: string;
};

export type AppSearchStatus = "idle" | "loading" | "ready";
