<h1>Hello {{full_name}}!</h1>

<p>This is a private page.  Only logged-in users can see it.</p>

{{#admin}}
<p>The custom claims example supports marking a user as an admin, and only admins will see this paragraph.</p>
{{/admin}}