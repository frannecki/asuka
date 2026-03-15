import { redirect } from "next/navigation";

export default async function LegacySessionChatRedirect({
  params,
}: {
  params: Promise<{ sessionId: string }>;
}) {
  const { sessionId } = await params;
  redirect(`/sessions/${sessionId}/chat`);
}
