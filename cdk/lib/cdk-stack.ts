import * as cdk from 'aws-cdk-lib';
import { Construct } from 'constructs';
import * as s3 from 'aws-cdk-lib/aws-s3';
import * as asg from 'aws-cdk-lib/aws-autoscaling';
import * as logs from 'aws-cdk-lib/aws-logs';
import * as ec2 from 'aws-cdk-lib/aws-ec2';
import * as ecr from 'aws-cdk-lib/aws-ecr';
import * as ecs from 'aws-cdk-lib/aws-ecs';
import * as iam from 'aws-cdk-lib/aws-iam';

export class CombustionStack extends cdk.Stack {
  constructor(scope: Construct, id: string, props?: cdk.StackProps) {
    super(scope, id, props);

    // S3 Bucket
    const name = this.node.tryGetContext('name');
    if (!name) {
      throw new Error('"name" is required');
    }
    const bucketName = `${name}-combustion`;
    const bucket = new s3.Bucket(this, 'CombustionBucket', {
      bucketName: bucketName,
      versioned: true,
    });

    // IAM Role for EC2
    const ec2Role = new iam.Role(this, 'InstanceRole', {
      assumedBy: new iam.ServicePrincipal('ec2.amazonaws.com'),
    });
    ec2Role.addManagedPolicy(iam.ManagedPolicy.fromAwsManagedPolicyName('AmazonSSMManagedInstanceCore'));

    // Allow EC2 to read from S3
    bucket.grantRead(ec2Role);

    const imageRepo = new ecr.Repository(this, 'Repository', {
      repositoryName: name,
      emptyOnDelete: true,
      removalPolicy: cdk.RemovalPolicy.DESTROY,
    });
    imageRepo.grantPull(ec2Role);

    // EC2 Instance
    const vpc = new ec2.Vpc(this, 'Vpc', {
      maxAzs: 2, // Adjust based on your requirements
      natGateways: 0, // Don't need a NAT Gateway since my EC2 instance has public IP
    });

    const securityGroup = new ec2.SecurityGroup(this, 'SecGroup', {
      vpc,
      securityGroupName: `${name}-secgroup`,
    });
    securityGroup.addIngressRule(ec2.Peer.anyIpv4(), ec2.Port.tcp(80), 'Allow HTTP traffic');

    const autoScalingGroup = new asg.AutoScalingGroup(this, 'ASG', {
      vpc,
      securityGroup,
      associatePublicIpAddress: true,
      instanceType: ec2.InstanceType.of(ec2.InstanceClass.T2, ec2.InstanceSize.MICRO),
      machineImage: ecs.EcsOptimizedImage.amazonLinux2023(),
      minCapacity: 1,
      maxCapacity: 1,
      role: ec2Role,
      vpcSubnets: { subnetType: ec2.SubnetType.PUBLIC },
    });


    const cluster = new ecs.Cluster(this, 'Cluster', {
      vpc,
    });
    const capacityProvider = new ecs.AsgCapacityProvider(this, 'AsgProvider', {
      autoScalingGroup,
      canContainersAccessInstanceRole: true,
    });
    cluster.addAsgCapacityProvider(capacityProvider);

    const taskRole = new iam.Role(this, 'TaskRole', {
      assumedBy: new iam.ServicePrincipal('ecs-tasks.amazonaws.com'),
    });
    taskRole.addManagedPolicy(iam.ManagedPolicy.fromAwsManagedPolicyName('AmazonS3FullAccess'));
    taskRole.addManagedPolicy(iam.ManagedPolicy.fromManagedPolicyArn(this, 'taskexecutionpolicy', 'arn:aws:iam::aws:policy/service-role/AmazonECSTaskExecutionRolePolicy'));

    const taskDefinition = new ecs.Ec2TaskDefinition(this, 'TaskDef', {
      networkMode: ecs.NetworkMode.BRIDGE,
      taskRole: taskRole,
    });
    const logGroup = new logs.LogGroup(this, 'LogGroup', {
      logGroupName: `/ecs/${name}-logs`,
      retention: logs.RetentionDays.TWO_WEEKS,
      removalPolicy: cdk.RemovalPolicy.DESTROY,
    });
    const container = taskDefinition.addContainer('Container', {
      image: ecs.ContainerImage.fromEcrRepository(imageRepo, 'latest'),
      memoryLimitMiB: 512,
      environment: {
        'RUST_LOG': 'info',
      },
      command: [
        bucketName,
      ],
      healthCheck: {
        command: ["CMD-SHELL", "curl -f http://localhost:8080/health || exit 1" ],
        interval: cdk.Duration.minutes(1),
        retries: 3,
        startPeriod: cdk.Duration.minutes(5),
        timeout: cdk.Duration.seconds(5),
      },
      logging: ecs.LogDrivers.awsLogs({
        streamPrefix: 'ecs',
        logGroup,
      }),
    });
    container.addPortMappings({
      containerPort: 8080,
      hostPort: 80,
    });

    const ecsService = new ecs.Ec2Service(this, 'Ec2Service', {
      cluster,
      taskDefinition,
    });


    // IAM User for Raspberry Pi
    const user = new iam.User(this, 'User');
    user.addManagedPolicy(iam.ManagedPolicy.fromAwsManagedPolicyName('AmazonS3FullAccess'));


    // Output the S3 bucket name
    new cdk.CfnOutput(this, 'Output', {
      value: bucket.bucketName,
    });
  }
}
