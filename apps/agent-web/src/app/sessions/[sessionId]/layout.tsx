import { SessionShell } from "@/components/session-shell";

export default async function SessionLayout({
  children,
  params,
}: {
  children: React.ReactNode;
  params: Promise<{ sessionId: string }>;
}) {
  const { sessionId } = await params;

  return <SessionShell sessionId={sessionId}>{children}</SessionShell>;
}
