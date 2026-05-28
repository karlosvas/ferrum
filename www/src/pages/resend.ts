import type { APIRoute } from "astro";
import { Resend } from "resend";

export const prerender = false;

const resend = new Resend(import.meta.env.RESEND_API_KEY);

export const POST: APIRoute = async ({ request }) => {
  const contentType = request.headers.get("content-type") ?? "";
  let name: string, email: string, subject: string, message: string;

  if (contentType.includes("application/json")) {
    const body = (await request.json()) as Record<string, string>;
    ({ name, email, subject, message } = body);
  } else if (
    contentType.includes("multipart/form-data") ||
    contentType.includes("application/x-www-form-urlencoded")
  ) {
    const form = await request.formData();
    name = (form.get("name") ?? "") as string;
    email = (form.get("email") ?? "") as string;
    subject = (form.get("subject") ?? "") as string;
    message = (form.get("message") ?? "") as string;
  } else {
    return new Response(
      JSON.stringify({ error: "Content-Type no soportado" }),
      {
        status: 415,
        headers: { "Content-Type": "application/json" },
      },
    );
  }

  if (!name || !email || !subject || !message) {
    return new Response(
      JSON.stringify({ error: "Todos los campos son obligatorios" }),
      {
        status: 400,
        headers: { "Content-Type": "application/json" },
      },
    );
  }

  const { error } = await resend.emails.send({
    from: "Ferrum Contact <onboarding@resend.dev>",
    to: ["carlosvassan@gmail.com"],
    replyTo: email,
    subject: `[Ferrum] ${subject}`,
    html: `
      <div style="font-family: sans-serif; max-width: 600px; margin: 0 auto; background: #0a0e1a; color: #e5e7eb; padding: 32px; border-radius: 12px;">
        <h2 style="color: #e85a3f; margin-top: 0;">Nuevo mensaje de contacto</h2>
        <table style="width: 100%; border-collapse: collapse;">
          <tr>
            <td style="padding: 8px 0; color: #9ca3af; width: 80px;">Nombre</td>
            <td style="padding: 8px 0; font-weight: 600;">${name}</td>
          </tr>
          <tr>
            <td style="padding: 8px 0; color: #9ca3af;">Email</td>
            <td style="padding: 8px 0;"><a href="mailto:${email}" style="color: #e85a3f;">${email}</a></td>
          </tr>
          <tr>
            <td style="padding: 8px 0; color: #9ca3af;">Asunto</td>
            <td style="padding: 8px 0;">${subject}</td>
          </tr>
        </table>
        <hr style="border-color: #ffffff10; margin: 20px 0;" />
        <p style="line-height: 1.7; white-space: pre-wrap;">${message}</p>
      </div>
    `,
  });

  if (error) {
    return new Response(JSON.stringify({ error: error.message }), {
      status: 500,
      headers: { "Content-Type": "application/json" },
    });
  }

  return new Response(JSON.stringify({ ok: true }), {
    status: 200,
    headers: { "Content-Type": "application/json" },
  });
};
