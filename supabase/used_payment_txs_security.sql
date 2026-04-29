-- Security hardening for public.used_payment_txs.
--
-- This table is written and read only by the backend using
-- SUPABASE_SERVICE_ROLE_KEY. Browser/client roles should not be able to
-- read, insert, update, or delete payment transaction records.

alter table public.used_payment_txs enable row level security;

revoke all on table public.used_payment_txs from anon;
revoke all on table public.used_payment_txs from authenticated;

drop policy if exists "used_payment_txs_no_anon_select" on public.used_payment_txs;
drop policy if exists "used_payment_txs_no_anon_insert" on public.used_payment_txs;
drop policy if exists "used_payment_txs_no_anon_update" on public.used_payment_txs;
drop policy if exists "used_payment_txs_no_anon_delete" on public.used_payment_txs;
drop policy if exists "used_payment_txs_no_authenticated_select" on public.used_payment_txs;
drop policy if exists "used_payment_txs_no_authenticated_insert" on public.used_payment_txs;
drop policy if exists "used_payment_txs_no_authenticated_update" on public.used_payment_txs;
drop policy if exists "used_payment_txs_no_authenticated_delete" on public.used_payment_txs;

create policy "used_payment_txs_no_anon_select"
on public.used_payment_txs
for select
to anon
using (false);

create policy "used_payment_txs_no_anon_insert"
on public.used_payment_txs
for insert
to anon
with check (false);

create policy "used_payment_txs_no_anon_update"
on public.used_payment_txs
for update
to anon
using (false)
with check (false);

create policy "used_payment_txs_no_anon_delete"
on public.used_payment_txs
for delete
to anon
using (false);

create policy "used_payment_txs_no_authenticated_select"
on public.used_payment_txs
for select
to authenticated
using (false);

create policy "used_payment_txs_no_authenticated_insert"
on public.used_payment_txs
for insert
to authenticated
with check (false);

create policy "used_payment_txs_no_authenticated_update"
on public.used_payment_txs
for update
to authenticated
using (false)
with check (false);

create policy "used_payment_txs_no_authenticated_delete"
on public.used_payment_txs
for delete
to authenticated
using (false);
