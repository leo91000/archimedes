pub const M000002_MIGRATION: &[&str] = &[
    r#"
        alter table :ARCHIMEDES_SCHEMA.jobs add column key text unique check(length(key) > 0);
    "#,
    r#"
        alter table :ARCHIMEDES_SCHEMA.jobs add locked_at timestamptz;
    "#,
    r#"
        alter table :ARCHIMEDES_SCHEMA.jobs add locked_by text;
    "#,
    // update any in-flight jobs
    r#"
        update :ARCHIMEDES_SCHEMA.jobs
            set locked_at = q.locked_at, locked_by = q.locked_by
            from :ARCHIMEDES_SCHEMA.job_queues q
            where q.queue_name = jobs.queue_name
            and q.locked_at is not null;
    "#,
    // update add_job behaviour to meet new requirements
    r#"
        drop function if exists :ARCHIMEDES_SCHEMA.add_job(
            identifier text,
            payload json,
            queue_name text,
            run_at timestamptz,
            max_attempts int
        );
    "#,
    r#"
        create function :ARCHIMEDES_SCHEMA.add_job(
            identifier text,
            payload json = '{}',
            queue_name text = null,
            run_at timestamptz = now(),
            max_attempts int = 25,
            job_key text = null
        ) returns :ARCHIMEDES_SCHEMA.jobs as $$
        declare
            v_job :ARCHIMEDES_SCHEMA.jobs;
        begin
            if job_key is not null then
                -- Upsert job
                insert into :ARCHIMEDES_SCHEMA.jobs (task_identifier, payload, queue_name, run_at, max_attempts, key)
                    values(
                        identifier,
                        payload,
                        queue_name,
                        run_at,
                        max_attempts,
                        job_key
                    )
                    on conflict (key) do update set
                        -- update all job details other than queue_name, which we want to keep
                        -- the same unless explicitly provided
                        task_identifier=excluded.task_identifier,
                        payload=excluded.payload,
                        queue_name=coalesce(add_job.queue_name, jobs.queue_name),
                        max_attempts=excluded.max_attempts,
                        run_at=excluded.run_at,
    
                        -- always reset error/retry state
                        attempts=0,
                        last_error=null
                    where jobs.locked_at is null
                    returning *
                    into v_job;

                -- If upsert succeeded (insert or update), return early
                if not (v_job is null) then
                    return v_job;
                end if;

                -- Upsert failed -> there must be an existing job that is locked. Remove
                -- existing key to allow a new one to be inserted, and prevent any
                -- subsequent retries by bumping attempts to the max allowed.
                update :ARCHIMEDES_SCHEMA.jobs
                    set
                        key = null,
                        attempts = jobs.max_attempts
                    where key = job_key;
            end if;

            -- insert the new job. Assume no conflicts due to the update above
            insert into :ARCHIMEDES_SCHEMA.jobs(task_identifier, payload, queue_name, run_at, max_attempts, key)
                values(
                    identifier,
                    payload,
                    queue_name,
                    run_at,
                    max_attempts,
                    job_key
                )
                returning *
                into v_job;

            return v_job;
        end;
        $$ language plpgsql volatile;
    "#,
    // implement new remove_job function
    r#"
        create function :ARCHIMEDES_SCHEMA.remove_job(
            job_key text
        ) returns :ARCHIMEDES_SCHEMA.jobs as $$
            delete from :ARCHIMEDES_SCHEMA.jobs
                where key = job_key
                and locked_at is null
            returning *;
        $$ language sql strict volatile;
    "#,
    // Update other functions to handle locked_at denormalisation
    r#"
        create or replace function :ARCHIMEDES_SCHEMA.get_job(worker_id text, task_identifiers text[] = null, job_expiry interval = interval '4 hours') returns :ARCHIMEDES_SCHEMA.jobs as $$
        declare
            v_job_id bigint;
            v_queue_name text;
            v_row :ARCHIMEDES_SCHEMA.jobs;
            v_now timestamptz = now();
        begin
            if worker_id is null or length(worker_id) < 10 then
                raise exception 'invalid worker id';
            end if;

            select job_queues.queue_name, jobs.id into v_queue_name, v_job_id
                from :ARCHIMEDES_SCHEMA.jobs
                inner join :ARCHIMEDES_SCHEMA.job_queues using (queue_name)
                where (job_queues.locked_at is null or job_queues.locked_at < (v_now - job_expiry))
                and run_at <= v_now
                and attempts < max_attempts
                and (task_identifiers is null or task_identifier = any(task_identifiers))
                order by priority asc, run_at asc, id asc
                limit 1
                for update of job_queues
                skip locked;

            if v_queue_name is null then
                return null;
            end if;

            update :ARCHIMEDES_SCHEMA.job_queues
                set
                    locked_by = worker_id,
                    locked_at = v_now
                where job_queues.queue_name = v_queue_name;

            update :ARCHIMEDES_SCHEMA.jobs
                set
                    attempts = attempts + 1,
                    locked_by = worker_id,
                    locked_at = v_now
                where id = v_job_id
                returning * into v_row;

            return v_row;
        end;
        $$ language plpgsql volatile;
    "#,
    // I was unsuccessful, re-schedule the job please
    r#"
        create or replace function :ARCHIMEDES_SCHEMA.fail_job(worker_id text, job_id bigint, error_message text) returns :ARCHIMEDES_SCHEMA.jobs as $$
        declare
            v_row :ARCHIMEDES_SCHEMA.jobs;
        begin
            update :ARCHIMEDES_SCHEMA.jobs
                set
                    last_error = error_message,
                    run_at = greatest(now(), run_at) + (exp(least(attempts, 10))::text || ' seconds')::interval,
                    locked_by = null,
                    locked_at = null
                where id = job_id and locked_by = worker_id
                returning * into v_row;

            update :ARCHIMEDES_SCHEMA.job_queues
                set locked_by = null, locked_at = null
                where queue_name = v_row.queue_name and locked_by = worker_id;

            return v_row;
        end;
        $$ language plpgsql volatile strict;
    "#,
];
