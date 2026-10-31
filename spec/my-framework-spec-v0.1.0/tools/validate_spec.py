#!/usr/bin/env python3
"""Valida los documentos del paquete; no ejecuta un runtime del framework."""
from __future__ import annotations

import copy
import datetime
import json
from pathlib import Path
import re
import tomllib
from urllib.parse import unquote, urldefrag, urlparse

import yaml
from jsonschema import Draft202012Validator, FormatChecker
from openapi_spec_validator import validate as validate_openapi
from referencing import Registry, Resource

ROOT = Path(__file__).resolve().parents[1]
SCHEMA_PATH = ROOT / 'schemas/contracts.schema.json'
SCHEMA = json.loads(SCHEMA_PATH.read_text(encoding='utf-8'))
DEFS = SCHEMA['$defs']
SCHEMA_URI = SCHEMA_PATH.as_uri()
REGISTRY = Registry().with_resource(SCHEMA_URI, Resource.from_contents(SCHEMA))
FORMATS = FormatChecker()


def ensure(condition, message):
    if not condition:
        raise AssertionError(message)


def validator(name):
    ensure(name in DEFS, f'Contrato desconocido: {name}')
    return Draft202012Validator({'$ref': SCHEMA_URI + '#/$defs/' + name}, registry=REGISTRY, format_checker=FORMATS)


def validate_instance(name, instance, label):
    errors = sorted(validator(name).iter_errors(instance), key=lambda e: str(list(e.absolute_path)))
    ensure(not errors, f'{label}: ' + '; '.join(f'{list(e.absolute_path)}: {e.message}' for e in errors[:3]))


def walk(value):
    yield value
    if isinstance(value, dict):
        for item in value.values():
            yield from walk(item)
    elif isinstance(value, list):
        for item in value:
            yield from walk(item)


def check_json_portable(value, label):
    for item in walk(value):
        ensure(not isinstance(item, (datetime.datetime, datetime.date, datetime.time)), f'{label}: fecha TOML nativa no portable')
        if isinstance(item, int) and not isinstance(item, bool):
            ensure(abs(item) <= 2**53 - 1, f'{label}: entero no seguro')
        if isinstance(item, float):
            import math
            ensure(math.isfinite(item), f'{label}: número no finito')


def type_semantics(document, by_id):
    kind = document['kind']
    if kind == 'Entity':
        for field in document['fields'].values():
            target = by_id.get(field['target'])
            ensure(target is not None and target['kind'] == 'ValueObject', f"{document['id']}: la referencia de entidad debe apuntar a un VO conocido")
    if kind == 'Relation':
        for end in ('left', 'right'):
            endpoint = document[end]
            ensure(endpoint['target'] in by_id and by_id[endpoint['target']]['kind'] in {'Entity','ValueObject'}, 'Extremo de relación desconocido')
            card = endpoint['cardinality']
            ensure(card.get('max',card['min']) >= card['min'], 'Cardinalidad inconsistente')
    if kind == 'ValueObject':
        for field in document['fields'].values():
            if 'format' in field:
                ensure(field['type'] == 'string', 'Formato requiere string')
            if 'min_length' in field or 'max_length' in field or 'pattern' in field:
                ensure(field['type'] == 'string', 'Longitud o patrón requiere string')
            if 'minimum' in field or 'maximum' in field:
                ensure(field['type'] in {'number','integer'}, 'Rango requiere número')
            if 'min_length' in field and 'max_length' in field:
                ensure(field['min_length'] <= field['max_length'], 'Longitudes inconsistentes')
            if 'minimum' in field and 'maximum' in field:
                ensure(field['minimum'] <= field['maximum'], 'Rango inconsistente')
            if 'pattern' in field:
                re.compile(field['pattern'])
                ensure(not re.search(r'\(\?[=!<]|\\[1-9]',field['pattern']), 'Patrón no portable')


def select_semantics(type_id, selected, by_id):
    for path in selected:
        parts = path.split('.')
        current = by_id[type_id]
        for pos, part in enumerate(parts):
            if part == 'id' and pos == len(parts)-1:
                continue
            field = current.get('fields',{}).get(part)
            ensure(field is not None, f'Campo seleccionado no existe: {path}')
            if pos < len(parts)-1:
                ensure(field['type']=='reference','Solo se expanden referencias tipadas')
                current = by_id[field['target']]


def main():
    Draft202012Validator.check_schema(SCHEMA)
    parsed_json={p:json.loads(p.read_text(encoding='utf-8')) for p in ROOT.rglob('*.json')}
    parsed_toml={p:tomllib.loads(p.read_text(encoding='utf-8')) for p in ROOT.rglob('*.toml')}
    parsed_yaml={p:yaml.safe_load(p.read_text(encoding='utf-8')) for p in ROOT.rglob('*.yaml')}
    for p,data in parsed_toml.items():
        check_json_portable(data,str(p.relative_to(ROOT)))
    for p,data in parsed_json.items():
        check_json_portable(data,str(p.relative_to(ROOT)))

    positive = 0
    definition_models={'ValueObject':'ValueObjectDefinition','Entity':'EntityDefinition','Relation':'RelationDefinition',
        'Event':'EventDefinition','Command':'CommandDefinition','Handler':'HandlerDefinition','Subscription':'SubscriptionDefinition',
        'Profile':'ProfileDefinition','Policy':'PolicyDefinition','View':'ViewDefinition'}
    definition_paths=sorted((ROOT/'examples/definitions').glob('*.toml'))
    documents=[parsed_toml[p] for p in definition_paths]
    by_id={d['id']:d for d in documents}
    ensure(len(by_id)==len(documents),'ID de definición duplicado')
    for p,d in zip(definition_paths,documents):
        validate_instance(definition_models[d['kind']],d,str(p.relative_to(ROOT)))
        type_semantics(d,by_id)
        positive+=1
        if d['kind'] in {'Event','Command'}:
            name=d.get('data_schema',d.get('input_schema'))
            ensure(name in DEFS,'Contrato de payload sin resolver')
        if d['kind']=='Command':
            ensure(d['owner'] in by_id and by_id[d['owner']]['kind']=='Handler','Dueño de comando no resuelto')
            ensure(d['message_name'] in by_id[d['owner']]['consumes'],'Owner no consume su comando')
        if d['kind']=='Subscription':
            ensure(d['handler'] in by_id and by_id[d['handler']]['kind']=='Handler','Handler de suscripción no resuelto')
            ensure(set(d['message_types'])<=set(by_id[d['handler']]['consumes']),'Suscripción incompatible con handler')
        if d['kind']=='View':
            ensure(d['query']['type'] in by_id,'Tipo de consulta sin resolver')
            select_semantics(d['query']['type'],d['query']['select'],by_id)
            if 'template' in d:
                template=ROOT/'examples'/d['template']
                ensure(template.is_file(),'Plantilla ausente')
        if d['kind']=='Policy':
            for rule in d['rules']:
                ensure(all(p in by_id and by_id[p]['kind']=='Profile' for p in rule['subject_profiles']),'Perfil de política no resuelto')
                ensure(all(t in by_id for t in rule.get('resource_types',[])),'Recurso de política no resuelto')

    project=parsed_toml[ROOT/'examples/project.toml']
    validate_instance('ProjectConfig',project,'project.toml');positive+=1
    ensure(set(project['definitions'])=={str(p.relative_to(ROOT/'examples')) for p in definition_paths},'Lista de definiciones de proyecto incompleta')
    ensure(project['limits']['snapshot_seconds']>=project['limits']['cursor_seconds'],'Snapshot dura menos que cursor')
    ensure(project['limits']['max_hops']<=16,'El perfil de sobres inicial admite hasta 16 saltos')
    engine=parsed_toml[ROOT/'examples/engines/sqlite.toml']
    validate_instance('EngineDefinition',engine,'sqlite.toml');positive+=1
    ensure(engine['certification']=='proposed','El paquete no certifica un driver')
    ensure(all(b['engine']==engine['id'] for b in project['bindings'].values()),'Motor del binding no resuelto')
    ensure(len({json.dumps(b['credentials'],sort_keys=True) for b in project['bindings'].values()})==1,
           'La composición de referencia requiere una misma base para sus unidades atómicas')
    validate_instance('Binding',parsed_toml[ROOT/'examples/bindings/s3-blob.toml'],'s3-blob.toml');positive+=1

    examples={
        'examples/messages/create-object.request.json':'CreateObjectInput',
        'examples/messages/update-object.request.json':'UpdateObjectInput',
        'examples/messages/command.json':'Command','examples/messages/object-created.event.json':'Event',
        'examples/messages/operation-succeeded.json':'Operation','examples/messages/operation-rejected.json':'Operation',
        'examples/messages/accepted.json':'Accepted','examples/messages/object.json':'ObjectDocument',
        'examples/messages/query.request.json':'QueryInput','examples/messages/query.response.json':'QueryResult',
        'examples/messages/release.request.json':'ReleaseInput','examples/session.json':'PublicSession',
        'examples/view-model.json':'ViewModel','examples/membership.json':'Membership',
        'examples/codecs/money.canonical.json':'ValueObjectDefinition',
    }
    for path,name in examples.items():
        validate_instance(name,parsed_json[ROOT/path],path);positive+=1
    ensure(set(examples)=={str(p.relative_to(ROOT)) for p in parsed_json if ROOT/'examples' in p.parents},'Ejemplo JSON sin contrato explícito')
    ensure(parsed_json[ROOT/'examples/codecs/money.canonical.json']==by_id['sales.money'],'TOML y JSON equivalente difieren')
    event=parsed_json[ROOT/'examples/messages/object-created.event.json']
    ensure(event['aggregateversion']==event['data']['revision'],'Revisión de evento inconsistente')
    ensure(event['data']['values']==parsed_json[ROOT/'examples/messages/object.json']['values'],'After-image difiere del objeto')
    query=parsed_json[ROOT/'examples/messages/query.request.json']
    select_semantics('sales.order',query['select'],by_id)
    ensure(set(parsed_json[ROOT/'examples/view-model.json']['draft'])=={'money_id'},'Borrador no coincide con vista')

    catalog=parsed_toml[ROOT/'events/catalog.toml']
    event_types={e['type'] for e in catalog['events']}
    handlers={h['id']:h for h in catalog['handlers']}
    commands={c['name']:c for c in catalog['commands']}
    ensure(len(event_types)==len(catalog['events']),'Evento de catálogo duplicado')
    ensure(len(commands)==len(catalog['commands']),'Comando de catálogo duplicado')
    for entry in catalog['events']:
        ensure(entry['data_schema'] in DEFS,'Esquema de evento de catálogo sin resolver')
    for entry in catalog['commands']:
        ensure(entry['input_schema'] in DEFS,'Esquema de comando de catálogo sin resolver')
        ensure(entry['owner'] in handlers,'Owner de catálogo no resuelto')
        ensure(entry['name'] in handlers[entry['owner']]['consumes'],'Owner de catálogo no consume comando')
        ensure(set(entry['emits'])<=event_types,'Evento emitido sin catálogo')
    for d in documents:
        if d['kind']=='Handler':
            ensure(set(d['consumes']) <= event_types|set(commands),'Mensaje consumido no resuelto')
            ensure(set(d['emits']) <= event_types,'Mensaje emitido no resuelto')
        if d['kind']=='Command':
            ensure(commands[d['message_name']]['owner']==d['owner'],'Owner difiere entre catálogos')
        if d['kind']=='Event':
            ensure(d['message_type'] in event_types,'Definición de evento sin catálogo')

    negative=parsed_json[ROOT/'conformance/invalid-instances.json']
    for case in negative:
        ensure(not validator(case['schema']).is_valid(case['value']),f"Caso inválido aceptado: {case['id']}")

    semantic_negative=0
    for altered in [
        {**by_id['sales.order'],'fields':{'total':{'type':'reference','target':'sales.unknown','required':True,'nullable':False}}},
        {**by_id['sales.customer_order'],'right':{'target':'sales.order','cardinality':{'min':2,'max':1}}},
    ]:
        try:type_semantics(altered,by_id)
        except AssertionError:semantic_negative+=1
        else:raise AssertionError('Caso semántico inválido aceptado')
    for value in [{'n':float('nan')},{'n':2**53},{'date':datetime.date(2026,10,22)}]:
        try:check_json_portable(value,'negative')
        except AssertionError:semantic_negative+=1
        else:raise AssertionError('Valor no portable aceptado')

    api=parsed_yaml[ROOT/'api/openapi.yaml']
    validate_openapi(api,base_uri=(ROOT/'api/openapi.yaml').as_uri())
    operation_ids=[]
    for path,item in api['paths'].items():
        for method,operation in item.items():
            if method not in {'get','post','put','patch','delete'}:continue
            operation_ids.append(operation['operationId'])
            if operation['operationId'] in {'dispatchCommand','createObject','updateObject','deleteObject','publishDefinitionRelease'}:
                ensure('202' in operation['responses'],'Mutación sin aceptación asíncrona')
    ensure(len(set(operation_ids))==len(operation_ids),'operationId duplicado')
    # Resolver cada referencia del documento y la biblioteca sin acceso de red.
    refs=0
    for path,data in [(ROOT/'api/openapi.yaml',api),(SCHEMA_PATH,SCHEMA)]:
        for item in walk(data):
            if not isinstance(item,dict) or '$ref' not in item:continue
            refs+=1
            filepart,pointer=urldefrag(item['$ref'])
            if filepart:
                ensure(not urlparse(filepart).scheme,'Referencia remota no esperada')
                target_path=(path.parent/filepart).resolve()
                ensure(ROOT in target_path.parents,'Referencia fuera del paquete')
                target=json.loads(target_path.read_text())
            else:target=data
            for segment in pointer.removeprefix('/').split('/') if pointer else []:
                key=unquote(segment).replace('~1','/').replace('~0','~')
                target=target[int(key)] if isinstance(target,list) else target[key]

    req=parsed_toml[ROOT/'requirements.toml']['requirements']
    ids=[r['id'] for r in req]
    ensure(len(ids)==len(set(ids)),'Requisito duplicado')
    feature=(ROOT/'conformance/scenarios.feature').read_text()
    tags=re.findall(r'@REQ-\d{3}',feature)
    ensure(set(ids)=={t[1:] for t in tags},'Requisitos sin escenarios o etiquetas sin requisito')
    ensure(len(tags)==len(ids),'Trazabilidad del ejemplo debe ser uno a uno')
    ensure(len(re.findall(r'^  Escenario:',feature,re.M))==len(ids),'Escenarios incompletos')

    links=0
    for p in ROOT.rglob('*.md'):
        for link in re.findall(r'\[[^\]]+\]\(([^)]+)\)',p.read_text()):
            if urlparse(link).scheme or link.startswith('#'):continue
            target=link.split('#')[0]
            ensure((p.parent/target).is_file(),f'Enlace roto: {p.name}: {link}')
            links+=1

    return {'status':'pass','schema_definitions':len(DEFS),'toml_files':len(parsed_toml),'json_files':len(parsed_json),
        'yaml_files':len(parsed_yaml),'positive_instances':positive,'negative_structural_instances':len(negative),
        'negative_semantic_instances':semantic_negative,'resolved_references':refs,'api_paths':len(api['paths']),
        'http_operations':len(operation_ids),'requirements':len(ids),'specified_scenarios':len(tags),'local_links':links,
        'openapi_validation':'passed','runtime_scenarios_executed':0,'toon_parser_executed':False,
        'aws_adapters_tested':False,'jcs_implementation_tested':False}


if __name__=='__main__':
    try:
        print(json.dumps(main(),indent=2,ensure_ascii=False))
    except Exception as exc:
        raise SystemExit(f'ERROR: {type(exc).__name__}: {exc}')
